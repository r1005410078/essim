"""运行example串口交互脚本，通过自建PTY连接正式ESSim从站；不访问硬件。"""
import json,os,selectors,subprocess,sys,tempfile,threading,tty
from pathlib import Path
from verify_tcp_slave_runtime import snapshot,point
ROOT=Path(__file__).resolve().parents[1]

def main():
    a,sa=os.openpty();b,sb=os.openpty();tty.setraw(sa);tty.setraw(sb)
    stop=threading.Event();errors=[]
    def bridge():
        try:
            with selectors.DefaultSelector() as sel:
                sel.register(a,1,b);sel.register(b,1,a)
                while not stop.is_set():
                    for key,_ in sel.select(.01):os.write(key.data,os.read(key.fd,512))
        except Exception as error:
            if not stop.is_set():errors.append(str(error))
    thread=threading.Thread(target=bridge,daemon=True);thread.start();server=None
    try:
        with tempfile.TemporaryDirectory(prefix='essim-rtu-example-') as temp:
            temp=Path(temp);s=snapshot('example',0,units=1)
            s['endpoint']['transport']=dict(kind='rtu',path=os.ttyname(sa),baud=19200,parity='none',dataBits=8,stopBits=2)
            model=s['values']['example-u1']['model']
            for area in ['holding','coil']:
                model['points'] += [point(f'{area}{i}',area,100+i,0,True) for i in range(4)]
            model['points'].append(point('protected','holding',200,9,False))
            source=temp/'snapshot.json';source.write_text(json.dumps([s]))
            server=subprocess.Popen([str(ROOT/'target/debug/examples/tcp_slave'),str(source),str(temp/'runs')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            assert json.loads(server.stdout.readline())['state']=='running'
            assert server.stdout.readline().strip()=='READY'
            commands=[os.ttyname(sb),'read','10','ri','0','rc','0','rd','0',
                      'yt','100-103','11,22,33,44','rh','100-103','yk','100-103','1','rc','100-103',
                      'yt','100','55','read','100','yk','100','0','rc','100',
                      'read','99','yt','200','99','read','200','q']
            process=subprocess.run([sys.executable,str(ROOT/'example/modbus_rtu_client.py'),'--timeout','.4'],input='\n'.join(commands)+'\n',capture_output=True,text=True,timeout=20)
            assert process.returncode==0,(process.stdout,process.stderr)
            for expected in ['holding[10] = 7','input[0] = 33','coil[0] = True','discrete[0] = False',
                             'holding[100] = 11','holding[101] = 22','holding[102] = 33','holding[103] = 44',
                             'coil[103] = True','holding[100] = 55','coil[100] = False','读取失败：','写入被拒绝：','holding[200] = 9']:
                assert expected in process.stdout,(expected,process.stdout)
            # 相同串口可再次打开，EOF正常退出。
            again=subprocess.run([sys.executable,str(ROOT/'example/modbus_rtu_client.py'),'--port',os.ttyname(sb)],input='',capture_output=True,text=True,timeout=5)
            assert again.returncode==0 and '串口已关闭' in again.stdout,again
            # 不存在站号的请求超时应报错；脚本不循环重发。
            missing=subprocess.run([sys.executable,str(ROOT/'example/modbus_rtu_client.py'),'--port',os.ttyname(sb),'--unit','2','--timeout','.1'],input='read\n0\n',capture_output=True,text=True,timeout=5)
            assert missing.returncode==1 and '通信失败' in missing.stderr,missing
            assert not errors,errors
            print(json.dumps(dict(transport='PTY RTU',script='example/modbus_rtu_client.py',interactivePortSelection=True,fourAreaReads=True,singleAndRangeWrites=True,exceptionsReported=True,protectedValueUnchanged=True,eofClosesPort=True,missingUnitTimeout=True,hardwareValidated=False),ensure_ascii=False))
    finally:
        if server and server.poll() is None:
            server.stdin.write('quit\n');server.stdin.flush()
            try:server.wait(timeout=10)
            except subprocess.TimeoutExpired:server.kill();server.wait()
        stop.set();thread.join(1)
        for fd in [a,sa,b,sb]:os.close(fd)
if __name__=='__main__':main()
