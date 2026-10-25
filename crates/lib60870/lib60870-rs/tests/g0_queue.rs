#![cfg(feature = "g0-monitor")]
//! G0真实TCP队列探针；原始字节客户端不调用lib60870客户端或ASDU解码API。
//! CA=47、IOA=100起及值0/1/2/3为测试约定，不代表设备点表。
use lib60870_rs::{Asdu, Cause, Interrogation, Point, Quality, Server, SessionToken};
use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    rc::Rc,
    time::{Duration, Instant},
};

fn address() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}
fn data(value: f32, cause: Cause, large: bool) -> Asdu {
    let mut asdu = Asdu::new(47, cause, false).unwrap();
    for index in 0..if large { 28 } else { 1 } {
        asdu.add(Point::measured(100 + index, value, Quality::GOOD).unwrap())
            .unwrap();
    }
    asdu
}
// G0限定应用调度器：按每ASDU最多255字节保守计费，最多128项；生产实现留到G_B。
struct MonitorFifo {
    queue: VecDeque<Asdu>,
    cut: Option<(u64, u64)>,
    position: Option<(u64, u64)>,
    desynced: bool,
    superseded: usize,
}
impl MonitorFifo {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            cut: None,
            position: None,
            desynced: false,
            superseded: 0,
        }
    }
    fn gap<T>(&mut self) -> Result<T, &'static str> {
        self.desynced = true;
        Err("ProjectionGap")
    }
    fn reserve_snapshot(
        &mut self,
        epoch: u64,
        cursor: u64,
        batch: Vec<Asdu>,
    ) -> Result<(), &'static str> {
        if self.desynced || batch.len() > 128 - self.queue.len() {
            return self.gap();
        }
        if self
            .position
            .is_some_and(|(e, last)| e != epoch || cursor < last)
        {
            return self.gap();
        }
        self.queue.extend(batch);
        self.cut = Some((epoch, cursor));
        self.position = Some((epoch, cursor));
        Ok(())
    }
    // 一个完整changes结果对应一个原子批次；单次提交产生多帧时必须一起准入。
    fn changes(
        &mut self,
        epoch: u64,
        after: u64,
        through: u64,
        batch: Vec<Asdu>,
    ) -> Result<bool, &'static str> {
        if self.desynced || through < after {
            return self.gap();
        }
        // changes查询可能没有新提交；当前epoch／位置的空轮询不是断档，不能触发重复重同步。
        if through == after {
            if !batch.is_empty()
                || self
                    .position
                    .is_some_and(|(e, last)| e != epoch || after != last)
            {
                return self.gap();
            }
            self.position = Some((epoch, after));
            return Ok(false);
        }
        if let Some((cut_epoch, cut)) = self.cut {
            if epoch != cut_epoch {
                return self.gap();
            }
            if through <= cut {
                self.superseded += batch.len();
                return Ok(false);
            }
        }
        if self
            .position
            .is_some_and(|(e, last)| e != epoch || after != last)
        {
            return self.gap();
        }
        if batch.len() > 128 - self.queue.len() {
            return self.gap();
        }
        self.queue.extend(batch);
        self.position = Some((epoch, through));
        Ok(true)
    }
    // 旧G0单帧用例的便捷入口；生产契约以完整changes批次为边界。
    fn change(&mut self, epoch: u64, cursor: u64, asdu: Asdu) -> Result<bool, &'static str> {
        let after = if self.cut.is_some_and(|(e, cut)| e == epoch && cursor <= cut) {
            cursor.saturating_sub(1)
        } else {
            self.position.map_or(0, |(_, last)| last)
        };
        self.changes(epoch, after, cursor, vec![asdu])
    }
}
// 仅G0所有者设施：会话失效时废弃应用积压，重新捕获当前状态；不访问C内部缓冲。
struct SessionProjection {
    session: Option<SessionToken>,
    fifo: MonitorFifo,
    superseded: usize,
    gaps: usize,
    snapshots: usize,
}
impl SessionProjection {
    fn new() -> Self {
        Self {
            session: None,
            fifo: MonitorFifo::new(),
            superseded: 0,
            gaps: 0,
            snapshots: 0,
        }
    }
    // 必须在每次Server.tick之后、处理快照回复／changes和发送之前执行。
    fn sync_session(
        &mut self,
        server: &Server,
        snapshot: impl FnOnce() -> (u64, u64, Vec<Asdu>),
    ) -> Result<bool, &'static str> {
        if self
            .session
            .as_ref()
            .is_some_and(|token| server.session_is_current(token))
            && !self.fifo.desynced
        {
            return Ok(false);
        }
        let changed = self.session.is_some() || self.fifo.desynced;
        if changed {
            self.gaps += 1;
            self.superseded += self.fifo.queue.len();
            self.session = None;
            // 传输失步只替代待发帧；同一状态来源的高水位必须跨Gap、会话和失败重试保留。
            self.fifo = MonitorFifo {
                position: self.fifo.position,
                ..MonitorFifo::new()
            };
        }
        let Some(session) = server.active_sessions().into_iter().next() else {
            return Ok(changed);
        };
        let (epoch, cursor, batch) = snapshot();
        self.fifo.reserve_snapshot(epoch, cursor, batch)?;
        self.session = Some(session);
        self.snapshots += 1;
        Ok(true)
    }
    #[allow(clippy::too_many_arguments)] // 受限测试设施直接展示会话与changes范围，尚非生产请求类型。
    fn changes(
        &mut self,
        server: &Server,
        source_session: &SessionToken,
        epoch: u64,
        after: u64,
        through: u64,
        batch: Vec<Asdu>,
    ) -> Result<bool, &'static str> {
        if !server.session_is_current(source_session)
            || self
                .session
                .as_ref()
                .is_none_or(|token| !server.session_is_current(token))
        {
            return Err("SessionInvalid");
        }
        self.fifo.changes(epoch, after, through, batch)
    }
    fn pump(&mut self, server: &mut Server, limit: usize) -> Result<usize, &'static str> {
        if self.fifo.desynced {
            return Err("ProjectionGap");
        }
        let token = self.session.as_ref().ok_or("SessionInvalid")?;
        if !server.session_is_current(token) {
            return Err("SessionInvalid");
        }
        let mut accepted = 0;
        for _ in 0..limit {
            let Some(head) = self.fifo.queue.front() else {
                break;
            };
            match server.try_send_to(token, head) {
                Ok(()) => {
                    self.fifo.queue.pop_front();
                    accepted += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => return Err("SessionInvalid"),
            }
        }
        Ok(accepted)
    }
}
#[test]
fn gap_resync_rejects_late_snapshots_and_preserves_progress_across_session_reset() {
    for reset_session in [false, true] {
        let address = address();
        let mut server = Server::bind(address, |_| false).unwrap();
        let mut peer = activated(&mut server, address);
        let mut owner = SessionProjection::new();
        owner
            .sync_session(&server, || {
                (1, 10, vec![data(1., Cause::Spontaneous, false)])
            })
            .unwrap();
        owner.pump(&mut server, 2).unwrap();
        until(&mut server, &mut peer, |_, p| p.frames.len() == 1);
        let session = server.active_sessions().remove(0);
        owner
            .changes(
                &server,
                &session,
                1,
                10,
                12,
                vec![data(3., Cause::Spontaneous, false)],
            )
            .unwrap();
        owner.pump(&mut server, 2).unwrap();
        until(&mut server, &mut peer, |_, p| p.frames.len() == 2);
        // 高水位包含已接纳但尚未发送的cursor13，不能只保留线上cursor12。
        owner
            .changes(
                &server,
                &session,
                1,
                12,
                13,
                vec![data(4., Cause::Spontaneous, false)],
            )
            .unwrap();
        assert_eq!(
            owner.changes(
                &server,
                &session,
                1,
                10,
                11,
                vec![data(2., Cause::Spontaneous, false)],
            ),
            Err("ProjectionGap")
        );
        if reset_session {
            let count = peer.controls.iter().filter(|c| **c == 11).count();
            peer.control(7);
            until(&mut server, &mut peer, |_, p| {
                p.controls.iter().filter(|c| **c == 11).count() > count
            });
            assert!(!server.session_is_current(&session));
        }
        // 连续失败的重同步也不能清掉高水位；换传输会话不等于换状态所有者。
        for (epoch, cut) in [(1, 10), (1, 12), (2, 1)] {
            assert_eq!(
                owner.sync_session(&server, || {
                    (epoch, cut, vec![data(1., Cause::Spontaneous, false)])
                }),
                Err("ProjectionGap")
            );
            assert_eq!(owner.fifo.position, Some((1, 13)));
            assert!(owner.fifo.queue.is_empty());
            assert!(owner.pump(&mut server, 2).is_err());
        }
        assert_eq!(owner.superseded, 1);
        let current = server.active_sessions().remove(0);
        assert_eq!(
            owner.changes(
                &server,
                &current,
                1,
                13,
                14,
                vec![data(5., Cause::Spontaneous, false)],
            ),
            Err("SessionInvalid"),
            "重同步尚未成功，不能恢复接纳变化"
        );
        owner
            .sync_session(&server, || {
                (1, 13, vec![data(4., Cause::Spontaneous, false)])
            })
            .unwrap();
        owner
            .changes(
                &server,
                &current,
                1,
                13,
                14,
                vec![data(5., Cause::Spontaneous, false)],
            )
            .unwrap();
        peer.ack();
        owner.pump(&mut server, 2).unwrap();
        until(&mut server, &mut peer, |_, p| p.frames.len() == 4);
        let values = peer
            .frames
            .iter()
            .map(|f| f32::from_le_bytes(f[9..13].try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(values, [1., 3., 4., 5.]);
        println!(
            "gap-resync reset-session={reset_session}: rejected cuts=10/12 and foreign epoch, retained=(1,13), wire={values:?}"
        );
    }
}

struct Peer {
    socket: TcpStream,
    buffer: Vec<u8>,
    frames: Vec<Vec<u8>>,
    controls: Vec<u8>,
    received: u16,
    sent: u16,
}
impl Peer {
    fn connect(address: SocketAddr) -> Self {
        let socket = TcpStream::connect(address).unwrap();
        socket.set_nonblocking(true).unwrap();
        Self {
            socket,
            buffer: vec![],
            frames: vec![],
            controls: vec![],
            received: 0,
            sent: 0,
        }
    }
    fn write(&mut self, bytes: &[u8]) {
        self.socket.write_all(bytes).unwrap();
    }
    fn control(&mut self, code: u8) {
        self.write(&[0x68, 4, code, 0, 0, 0]);
    }
    fn gi(&mut self) {
        // OA=17、test=true验证自有响应重建不丢头字段；QOI=20。
        let mut frame = vec![0x68, 14];
        frame.extend_from_slice(&(self.sent * 2).to_le_bytes());
        frame.extend_from_slice(&0u16.to_le_bytes()); // 故意不确认服务端I帧，制造k窗口堵塞。
        frame.extend_from_slice(&[100, 1, 6 | 0x80, 17, 47, 0, 0, 0, 0, 20]);
        self.sent += 1;
        self.write(&frame);
    }
    fn ack(&mut self) {
        let nr = (self.received * 2).to_le_bytes();
        self.write(&[0x68, 4, 1, 0, nr[0], nr[1]]);
    }
    fn read(&mut self) {
        let mut bytes = [0; 8192];
        loop {
            match self.socket.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => self.buffer.extend_from_slice(&bytes[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("wire read: {e}"),
            }
        }
        while self.buffer.len() >= 2 {
            assert_eq!(self.buffer[0], 0x68);
            let len = self.buffer[1] as usize;
            assert!((4..=253).contains(&len));
            if self.buffer.len() < len + 2 {
                break;
            }
            let body = self.buffer[2..len + 2].to_vec();
            self.buffer.drain(..len + 2);
            if body[0] & 1 == 0 {
                let ns = u16::from_le_bytes([body[0], body[1]]) / 2;
                assert_eq!(ns, self.received, "I帧不能重复或乱序");
                self.received += 1;
                self.frames.push(body[4..].to_vec());
            } else if body[0] & 3 == 3 {
                self.controls.push(body[0]);
            }
        }
    }
}
fn until(server: &mut Server, peer: &mut Peer, mut condition: impl FnMut(&Server, &Peer) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        server.tick().unwrap();
        peer.read();
        if condition(server, peer) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "超时，received={}",
            peer.received
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn activated(server: &mut Server, address: SocketAddr) -> Peer {
    let mut peer = Peer::connect(address);
    peer.control(7);
    // 必须消费首次STARTDT_CON；后续重置测试才能用新增确认准确对应本次请求。
    until(server, &mut peer, |s, p| {
        !s.active_sessions().is_empty() && p.controls.contains(&11)
    });
    peer
}
fn saturate_window(server: &mut Server, peer: &mut Peer) -> SessionToken {
    let session = server.active_sessions().remove(0);
    for _ in 0..12 {
        server
            .try_send_to(&session, &data(0., Cause::Spontaneous, false))
            .unwrap();
    }
    until(server, peer, |_, p| p.received == 12);
    peer.frames.clear();
    session
}

#[test]
fn mixed_priority_reproduces_old_spontaneous_after_new_gi() {
    let address = address();
    let handled = Rc::new(RefCell::new(false));
    let flag = handled.clone();
    let mut server = Server::bind(address, move |r| {
        r.confirm(false).unwrap();
        r.send(&data(2., Cause::Interrogated, false)).unwrap();
        r.finish().unwrap();
        *flag.borrow_mut() = true;
        true
    })
    .unwrap();
    let mut peer = activated(&mut server, address);
    saturate_window(&mut server, &mut peer);
    server
        .enqueue(&data(1., Cause::Spontaneous, false))
        .unwrap();
    peer.gi();
    until(&mut server, &mut peer, |_, _| *handled.borrow());
    assert!(peer.frames.is_empty()); // k=12且没有确认，业务回复仍在队列。
    peer.ack();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 4);
    assert_eq!(
        peer.frames.iter().map(|f| f[2] & 0x3f).collect::<Vec<_>>(),
        [7, 20, 10, 3]
    );
    assert_eq!(
        f32::from_le_bytes(peer.frames[1][9..13].try_into().unwrap()),
        2.
    );
    assert_eq!(
        f32::from_le_bytes(peer.frames[3][9..13].try_into().unwrap()),
        1.
    );
    println!("mixed-priority counterexample: ACT_CON, GI(new=2), ACT_TERM, spontaneous(old=1)");
}

#[test]
fn single_fifo_survives_window_backlog_cross_tick_gi_and_queue_rejection() {
    let address = address();
    let requests = Rc::new(RefCell::new(VecDeque::<Interrogation>::new()));
    let captured = requests.clone();
    let mut server = Server::bind(address, move |r| {
        captured
            .borrow_mut()
            .push_back(r.owned_interrogation().unwrap());
        true
    })
    .unwrap();
    let mut peer = activated(&mut server, address);
    let session = saturate_window(&mut server, &mut peer);
    // 未发旧自发帧也先走高队列，禁止与低队列混排。
    for _ in 0..8 {
        server
            .try_send_to(&session, &data(1., Cause::Spontaneous, true))
            .unwrap();
    }
    peer.gi();
    until(&mut server, &mut peer, |_, _| !requests.borrow().is_empty());
    let request = requests.borrow_mut().pop_front().unwrap();
    let mut scheduler = MonitorFifo::new();
    let mut batch = vec![request.confirmation(false).unwrap()];
    for _ in 0..100 {
        let mut part = request.data(false).unwrap();
        for ioa in 100..128 {
            part.add(Point::measured(ioa, 2., Quality::GOOD).unwrap())
                .unwrap();
        }
        batch.push(part);
    }
    batch.push(request.termination().unwrap());
    scheduler.reserve_snapshot(1, 10, batch).unwrap();
    assert!(
        !scheduler
            .change(1, 9, data(1., Cause::Spontaneous, true))
            .unwrap(),
        "异步迟到的切点前投影必须被快照替代"
    );
    // 切点后新变化只能排在整个总召批次之后。
    for cursor in 11..13 {
        scheduler
            .change(1, cursor, data(3., Cause::Spontaneous, true))
            .unwrap();
    }
    assert_eq!(scheduler.superseded, 1);
    let fifo = &mut scheduler.queue;
    let mut rejected = 0;
    let mut accepted = 0;
    let mut ticks = 0;
    while rejected == 0 {
        for _ in 0..2 {
            // 每tick限量推进，回调已经返回。
            let Some(head) = fifo.front() else {
                break;
            };
            match server.try_send_to(request.session(), head) {
                Ok(()) => {
                    fifo.pop_front();
                    accepted += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    rejected += 1;
                    break;
                }
                Err(e) => panic!("{e}"),
            }
        }
        server.tick().unwrap();
        peer.read();
        ticks += 1;
        assert!(ticks < 1000);
        assert!(peer.frames.is_empty());
    }
    assert!(!fifo.is_empty());
    peer.ack();
    let deadline = Instant::now() + Duration::from_secs(8);
    while peer.frames.len() < 112 {
        for _ in 0..2 {
            let Some(head) = fifo.front() else {
                break;
            };
            match server.try_send_to(request.session(), head) {
                Ok(()) => {
                    fifo.pop_front();
                    accepted += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    rejected += 1;
                    break;
                }
                Err(e) => panic!("{e}"),
            }
        }
        server.tick().unwrap();
        let before = peer.received;
        peer.read();
        if peer.received != before {
            peer.ack();
        }
        assert!(Instant::now() < deadline, "frames={}", peer.frames.len());
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(fifo.is_empty());
    assert_eq!(accepted, 104);
    assert!(rejected > 0);
    assert!(ticks > 1);
    for (index, frame) in peer.frames.iter().enumerate() {
        assert_eq!(&frame[4..6], &[47, 0]);
        let expected = if index < 8 {
            (3, 1.)
        } else if index == 8 {
            (7, 0.)
        } else if index < 109 {
            (20, 2.)
        } else if index == 109 {
            (10, 0.)
        } else {
            (3, 3.)
        };
        assert_eq!(frame[2] & 0x3f, expected.0, "index={index}");
        if frame[2] & 0x3f == 20 {
            assert_eq!(frame[3], 17);
            assert_eq!(frame[2] & 0x80, 0x80);
        }
        if frame[0] == 13 {
            assert_eq!(
                f32::from_le_bytes(frame[9..13].try_into().unwrap()),
                expected.1
            );
        } else {
            assert_eq!(frame[0], 100);
            assert_eq!(frame[3], 17);
            assert_eq!(frame[2] & 0x80, 0x80);
            assert_eq!(frame[9], 20);
        }
    }
    assert_eq!(server.queued(), 0, "单FIFO不得使用低队列");
    for index in [0, 8, 9, 109, 110] {
        println!(
            "wire ASDU[{index}]={}",
            peer.frames[index]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    println!(
        "single-FIFO: old=8, GI=100+2, new=2, accepted={accepted}, WouldBlock={rejected}, pre-ack ticks={ticks}; no duplicate, rollback or low-queue entry"
    );
}

#[test]
fn session_tokens_reject_foreign_stopped_deactivated_and_reconnected_owners() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let mut peer = activated(&mut server, address);
    let old = server.active_sessions().remove(0);
    let value = data(1., Cause::Spontaneous, false);
    let mut other = Server::bind(self::address(), |_| false).unwrap();
    assert_eq!(
        other.try_send_to(&old, &value).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    peer.control(0x13);
    until(&mut server, &mut peer, |s, _| {
        s.active_sessions().is_empty()
    });
    assert_eq!(
        server.try_send_to(&old, &value).unwrap_err().kind(),
        std::io::ErrorKind::NotConnected
    );
    peer.control(7);
    until(&mut server, &mut peer, |s, _| {
        !s.active_sessions().is_empty()
    });
    assert!(server.try_send_to(&old, &value).is_err());
    let reactivated = server.active_sessions().remove(0);
    drop(peer);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !server.active_sessions().is_empty() {
        server.tick().unwrap();
        assert!(Instant::now() < deadline);
    }
    assert!(server.try_send_to(&reactivated, &value).is_err());
    let mut peer = activated(&mut server, address);
    let current = server.active_sessions().remove(0);
    assert!(server.try_send_to(&old, &value).is_err());
    server
        .try_send_to(&current, &data(3., Cause::Interrogated, false))
        .unwrap();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 1);
    assert_eq!(
        f32::from_le_bytes(peer.frames[0][9..13].try_into().unwrap()),
        3.
    );
    server.stop();
    assert!(server.active_sessions().is_empty());
    assert!(server.try_send_to(&current, &value).is_err());
    drop(server);
    TcpListener::bind(address).unwrap();
}

#[test]
fn repeated_startdt_invalidates_token_when_upstream_resets_pending_high_queue() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let mut peer = activated(&mut server, address);
    let old = saturate_window(&mut server, &mut peer);
    server
        .try_send_to(&old, &data(1., Cause::Spontaneous, false))
        .unwrap();
    peer.control(7); // 上游即使已激活也会清高队列，却不再次发ACTIVATED事件。
    let confirmations = peer.controls.iter().filter(|code| **code == 11).count();
    until(&mut server, &mut peer, |_, p| {
        p.controls.iter().filter(|code| **code == 11).count() > confirmations
    });
    assert!(
        server
            .try_send_to(&old, &data(2., Cause::Spontaneous, false))
            .is_err(),
        "不能让已经跨过原生队列重置的token继续有效"
    );
}

#[test]
fn application_fifo_rejects_overflow_epoch_gap_and_partial_snapshot_admission() {
    let mut fifo = MonitorFifo::new();
    for cursor in 1..=128 {
        fifo.change(1, cursor, data(1., Cause::Spontaneous, false))
            .unwrap();
    }
    assert_eq!(
        fifo.change(1, 129, data(2., Cause::Spontaneous, false)),
        Err("ProjectionGap")
    );
    assert_eq!(fifo.queue.len(), 128);
    assert!(fifo.desynced);
    assert!(
        fifo.reserve_snapshot(1, 130, vec![data(3., Cause::Interrogated, false)])
            .is_err()
    );
    assert_eq!(fifo.queue.len(), 128);
    let mut fifo = MonitorFifo::new();
    fifo.reserve_snapshot(2, 1, vec![data(3., Cause::Interrogated, false)])
        .unwrap();
    assert_eq!(
        fifo.change(1, 999, data(1., Cause::Spontaneous, false)),
        Err("ProjectionGap")
    );
    assert_eq!(fifo.queue.len(), 1);
    assert!(fifo.desynced);
}

#[test]
fn owned_interrogation_rejects_out_of_scope_requests_and_builds_negative_reply() {
    let address = address();
    let outcomes = Rc::new(RefCell::new(Vec::new()));
    let captured = outcomes.clone();
    let mut server = Server::bind(address, move |r| {
        match r.owned_interrogation() {
            Ok(request) => {
                r.send(&request.confirmation(true).unwrap()).unwrap();
                captured.borrow_mut().push(true);
            }
            Err(_) => captured.borrow_mut().push(false),
        }
        true
    })
    .unwrap();
    let mut peer = activated(&mut server, address);
    for (index, (cot, qoi, ioa)) in [(6, 21, 0), (6, 20, 1), (6 | 0x40, 20, 0), (6, 20, 0)]
        .into_iter()
        .enumerate()
    {
        let mut frame = vec![0x68, 14];
        frame.extend_from_slice(&((index as u16) * 2).to_le_bytes());
        frame.extend_from_slice(&0u16.to_le_bytes());
        frame.extend_from_slice(&[100, 1, cot, 17, 47, 0, ioa, 0, 0, qoi]);
        peer.write(&frame);
        until(&mut server, &mut peer, |_, _| {
            outcomes.borrow().len() == index + 1
        });
    }
    until(&mut server, &mut peer, |_, p| p.frames.len() == 1);
    assert_eq!(*outcomes.borrow(), [false, false, false, true]);
    assert_eq!(peer.frames[0], [100, 1, 7 | 0x40, 17, 47, 0, 0, 0, 0, 20]);
    assert!(server.is_running());
}

#[test]
fn single_redundancy_group_takeover_invalidates_previous_active_token() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let first = activated(&mut server, address);
    let old = server.active_sessions().remove(0);
    let mut second = Peer::connect(address);
    second.control(7);
    until(&mut server, &mut second, |_, p| p.controls.contains(&11));
    assert_eq!(
        server
            .try_send_to(&old, &data(1., Cause::Spontaneous, false))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotConnected
    );
    assert_eq!(server.active_sessions().len(), 1);
    let new = server.active_sessions().remove(0);
    server
        .try_send_to(&new, &data(3., Cause::Interrogated, false))
        .unwrap();
    until(&mut server, &mut second, |_, p| p.frames.len() == 1);
    assert_eq!(
        f32::from_le_bytes(second.frames[0][9..13].try_into().unwrap()),
        3.
    );
    drop(first);
}

#[test]
fn all_upstream_startdt_reset_forms_invalidate_the_old_session() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let mut peer = activated(&mut server, address);
    saturate_window(&mut server, &mut peer);
    // 固定上游会接受这些非标准控制帧；测试防御性失效，不将其声明为合法104报文。
    for bytes in [
        &[0x68, 4, 7, 0, 1, 0][..],
        &[0x68, 4, 0x0f, 0, 0, 0],
        &[0x68, 5, 7, 0, 0, 0, 0],
        &[0x68, 4, 0x87, 0, 0, 0],
    ] {
        let old = server.active_sessions().remove(0);
        server
            .try_send_to(&old, &data(1., Cause::Spontaneous, false))
            .unwrap();
        let confirmations = peer.controls.iter().filter(|code| **code == 11).count();
        peer.write(bytes);
        until(&mut server, &mut peer, |_, p| {
            p.controls.iter().filter(|code| **code == 11).count() > confirmations
        });
        assert!(
            !server.session_is_current(&old),
            "重置帧{bytes:02x?}必须使旧会话失效"
        );
        assert!(
            server
                .try_send_to(&old, &data(2., Cause::Spontaneous, false))
                .is_err()
        );
    }
    let current = server.active_sessions().remove(0);
    server
        .try_send_to(&current, &data(3., Cause::Spontaneous, false))
        .unwrap();
    peer.ack();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 1);
    assert_eq!(
        f32::from_le_bytes(peer.frames[0][9..13].try_into().unwrap()),
        3.
    );
}
#[test]
fn session_query_detects_idle_reset_without_sending_an_asdu() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let mut peer = activated(&mut server, address);
    let old = server.active_sessions().remove(0);
    assert!(server.session_is_current(&old));
    let confirmations = peer.controls.iter().filter(|code| **code == 11).count();
    peer.control(7);
    until(&mut server, &mut peer, |_, p| {
        p.controls.iter().filter(|code| **code == 11).count() > confirmations
    });
    assert!(!server.session_is_current(&old));
    assert!(peer.frames.is_empty());
    let current = server.active_sessions().remove(0);
    assert!(server.session_is_current(&current));
    let other = Server::bind(self::address(), |_| false).unwrap();
    assert!(!other.session_is_current(&current));
    server.stop();
    assert!(!server.session_is_current(&current));
}
#[test]
fn fifo_rejects_postcut_regression_and_stale_snapshot() {
    let mut fifo = MonitorFifo::new();
    fifo.reserve_snapshot(1, 10, vec![data(1., Cause::Interrogated, false)])
        .unwrap();
    fifo.change(1, 12, data(3., Cause::Spontaneous, false))
        .unwrap();
    assert_eq!(
        fifo.change(1, 11, data(2., Cause::Spontaneous, false)),
        Err("ProjectionGap")
    );
    assert_eq!(fifo.queue.len(), 2);
    assert!(fifo.desynced);
    let mut fifo = MonitorFifo::new();
    fifo.change(1, 11, data(3., Cause::Spontaneous, false))
        .unwrap();
    assert_eq!(
        fifo.reserve_snapshot(1, 10, vec![data(2., Cause::Interrogated, false)]),
        Err("ProjectionGap")
    );
    assert_eq!(fifo.queue.len(), 1);
}

#[test]
fn changes_batches_are_contiguous_atomic_and_keep_multiple_frames_per_commit() {
    let mut fifo = MonitorFifo::new();
    fifo.reserve_snapshot(1, 10, vec![]).unwrap();
    assert!(
        fifo.changes(
            1,
            10,
            11,
            vec![
                data(3., Cause::Spontaneous, false),
                data(3., Cause::Spontaneous, false)
            ]
        )
        .unwrap()
    );
    assert_eq!(fifo.queue.len(), 2);
    assert_eq!(fifo.position, Some((1, 11)));
    assert_eq!(fifo.changes(1, 11, 11, vec![]), Ok(false));
    assert_eq!(fifo.queue.len(), 2);
    assert!(!fifo.desynced, "当前位置的空轮询不能把安静会话判成Gap");
    // 重叠批次、重复提交和跳过未确认游标都不能悄悄接续。
    for (after, through) in [(10, 12), (11, 11), (12, 13)] {
        let mut fifo = MonitorFifo::new();
        fifo.reserve_snapshot(1, 11, vec![]).unwrap();
        assert_eq!(
            fifo.changes(1, after, through, vec![data(2., Cause::Spontaneous, false)]),
            Err("ProjectionGap")
        );
        assert!(fifo.queue.is_empty());
    }
    let mut fifo = MonitorFifo::new();
    fifo.reserve_snapshot(1, 10, vec![]).unwrap();
    for through in 11..138 {
        fifo.change(1, through, data(3., Cause::Spontaneous, false))
            .unwrap();
    }
    assert_eq!(
        fifo.changes(
            1,
            137,
            138,
            vec![
                data(2., Cause::Spontaneous, false),
                data(2., Cause::Spontaneous, false)
            ]
        ),
        Err("ProjectionGap")
    );
    assert_eq!(fifo.queue.len(), 127);
    assert_eq!(fifo.position, Some((1, 137)));
}
#[test]
fn testfr_branch_does_not_invalidate_a_live_session() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let mut peer = activated(&mut server, address);
    let token = server.active_sessions().remove(0);
    // 0x47同时命中STARTDT位，但固定上游优先解释为TESTFR_ACT。
    peer.control(0x47);
    until(&mut server, &mut peer, |_, p| p.controls.contains(&0x83));
    assert!(server.session_is_current(&token));
    server
        .try_send_to(&token, &data(3., Cause::Spontaneous, false))
        .unwrap();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 1);
}

#[test]
fn pending_projection_restarts_from_fresh_snapshot_on_all_session_transitions() {
    for transition in [
        "disconnect",
        "stopdt",
        "repeat-startdt",
        "noncanonical-startdt",
        "takeover",
    ] {
        let address = address();
        let mut server = Server::bind(address, |_| false).unwrap();
        let mut peer = activated(&mut server, address);
        let old = saturate_window(&mut server, &mut peer);
        let mut owner = SessionProjection::new();
        owner
            .sync_session(&server, || {
                (1, 10, vec![data(1., Cause::Spontaneous, false)])
            })
            .unwrap();
        owner
            .changes(
                &server,
                &old,
                1,
                10,
                11,
                (0..90)
                    .map(|_| data(2., Cause::Spontaneous, true))
                    .collect(),
            )
            .unwrap();
        owner.pump(&mut server, 128).unwrap();
        let pending = owner.fifo.queue.len();
        assert!(pending > 0);
        match transition {
            "disconnect" => {
                drop(peer);
                let deadline = Instant::now() + Duration::from_secs(3);
                while server.session_is_current(&old) {
                    server.tick().unwrap();
                    assert!(Instant::now() < deadline);
                }
                owner
                    .sync_session(&server, || panic!("离线时不捕获快照"))
                    .unwrap();
                assert!(owner.fifo.queue.is_empty());
                peer = activated(&mut server, address);
            }
            "stopdt" => {
                peer.control(0x13);
                until(&mut server, &mut peer, |s, _| !s.session_is_current(&old));
                owner
                    .sync_session(&server, || panic!("未激活时不捕获快照"))
                    .unwrap();
                let count = peer.controls.iter().filter(|c| **c == 11).count();
                peer.control(7);
                until(&mut server, &mut peer, |_, p| {
                    p.controls.iter().filter(|c| **c == 11).count() > count
                });
            }
            "repeat-startdt" | "noncanonical-startdt" => {
                let count = peer.controls.iter().filter(|c| **c == 11).count();
                if transition == "repeat-startdt" {
                    peer.control(7);
                } else {
                    peer.write(&[0x68, 4, 7, 0, 1, 0]);
                }
                until(&mut server, &mut peer, |_, p| {
                    p.controls.iter().filter(|c| **c == 11).count() > count
                });
            }
            "takeover" => {
                let mut replacement = Peer::connect(address);
                replacement.control(7);
                until(&mut server, &mut replacement, |_, p| {
                    p.controls.contains(&11)
                });
                drop(peer);
                peer = replacement;
            }
            _ => unreachable!(),
        }
        assert!(!server.session_is_current(&old));
        // 旧会话丢失期间源状态已到cursor20；新快照必须使用最新值7，而不是积压的1/2。
        owner
            .sync_session(&server, || {
                (1, 20, vec![data(7., Cause::Spontaneous, false)])
            })
            .unwrap();
        assert_eq!(owner.snapshots, 2);
        assert_eq!(owner.gaps, 1);
        assert!(owner.superseded >= pending);
        assert_eq!(
            owner.changes(
                &server,
                &old,
                1,
                20,
                21,
                vec![data(2., Cause::Spontaneous, false)]
            ),
            Err("SessionInvalid")
        );
        let current = server.active_sessions().remove(0);
        owner
            .changes(
                &server,
                &current,
                1,
                20,
                21,
                vec![data(8., Cause::Spontaneous, false)],
            )
            .unwrap();
        peer.ack();
        owner.pump(&mut server, 2).unwrap();
        until(&mut server, &mut peer, |_, p| p.frames.len() == 2);
        let values = peer
            .frames
            .iter()
            .map(|f| f32::from_le_bytes(f[9..13].try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(values, [7., 8.], "{transition}不能向新代际回放旧积压");
        assert_eq!(server.queued(), 0);
        println!(
            "resync {transition}: old-app-frames-superseded={}, snapshots={}, gap={}, wire={values:?}",
            owner.superseded, owner.snapshots, owner.gaps
        );
    }
}
#[test]
fn idle_empty_fifo_reset_still_captures_and_sends_a_new_snapshot() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let mut peer = activated(&mut server, address);
    let mut owner = SessionProjection::new();
    owner
        .sync_session(&server, || {
            (1, 10, vec![data(1., Cause::Spontaneous, false)])
        })
        .unwrap();
    owner.pump(&mut server, 2).unwrap();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 1);
    peer.ack();
    assert!(owner.fifo.queue.is_empty());
    assert!(
        !owner
            .sync_session(&server, || panic!("会话未变不重新取快照"))
            .unwrap()
    );
    let count = peer.controls.iter().filter(|c| **c == 11).count();
    peer.control(7);
    until(&mut server, &mut peer, |_, p| {
        p.controls.iter().filter(|c| **c == 11).count() > count
    });
    owner
        .sync_session(&server, || {
            (1, 12, vec![data(3., Cause::Spontaneous, false)])
        })
        .unwrap();
    owner.pump(&mut server, 2).unwrap();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 2);
    assert_eq!(owner.snapshots, 2);
    assert_eq!(owner.gaps, 1);
    assert_eq!(
        f32::from_le_bytes(peer.frames[1][9..13].try_into().unwrap()),
        3.
    );
}
#[test]
fn late_postcut_changes_and_old_snapshot_cannot_roll_back_real_tcp_values() {
    let address = address();
    let mut server = Server::bind(address, |_| false).unwrap();
    let mut peer = activated(&mut server, address);
    let session = server.active_sessions().remove(0);
    let mut owner = SessionProjection::new();
    owner
        .sync_session(&server, || {
            (1, 10, vec![data(1., Cause::Spontaneous, false)])
        })
        .unwrap();
    owner.pump(&mut server, 2).unwrap();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 1);
    owner
        .changes(
            &server,
            &session,
            1,
            10,
            12,
            vec![data(3., Cause::Spontaneous, false)],
        )
        .unwrap();
    owner.pump(&mut server, 2).unwrap();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 2);
    assert_eq!(
        owner.changes(
            &server,
            &session,
            1,
            10,
            11,
            vec![data(2., Cause::Spontaneous, false)]
        ),
        Err("ProjectionGap")
    );
    assert!(owner.pump(&mut server, 2).is_err());
    owner
        .sync_session(&server, || {
            (1, 12, vec![data(3., Cause::Spontaneous, false)])
        })
        .unwrap();
    assert_eq!(
        owner
            .fifo
            .reserve_snapshot(1, 10, vec![data(2., Cause::Interrogated, false)]),
        Err("ProjectionGap")
    );
    assert!(owner.pump(&mut server, 2).is_err());
    owner
        .sync_session(&server, || {
            (1, 12, vec![data(3., Cause::Spontaneous, false)])
        })
        .unwrap();
    owner.pump(&mut server, 2).unwrap();
    until(&mut server, &mut peer, |_, p| p.frames.len() == 3);
    let values = peer
        .frames
        .iter()
        .map(|f| f32::from_le_bytes(f[9..13].try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(values, [1., 3., 3.]);
    println!(
        "late source batch/snapshot: gaps={}, wire={values:?}; stale=2 never sent",
        owner.gaps
    );
}
