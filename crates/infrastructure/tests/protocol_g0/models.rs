//! 配置身份与61850模型／服务的有限反例；不是生产模型编译器或原生互操作。
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
#[derive(Clone, Debug, PartialEq, Eq)]
struct Definition {
    model: String,
    asset: Vec<u8>,
}
#[derive(Clone)]
struct AppliedSnapshot {
    definition: Definition,
    generation: u64,
}
#[derive(Clone)]
struct RuntimeSnapshot(Definition);
#[derive(Clone)]
struct AppliedPlanToken {
    owner: Rc<()>,
    generation: u64,
}
struct Plans {
    owner: Rc<()>,
    saved: Definition,
    applied: AppliedSnapshot,
}
impl Plans {
    fn new() -> Self {
        let saved = Definition {
            model: "A".into(),
            asset: b"asset-A".to_vec(),
        };
        Self {
            owner: Rc::new(()),
            saved: saved.clone(),
            applied: AppliedSnapshot {
                definition: saved,
                generation: 1,
            },
        }
    }
    fn token(&self) -> AppliedPlanToken {
        AppliedPlanToken {
            owner: self.owner.clone(),
            generation: self.applied.generation,
        }
    }
    fn start(&self, token: &AppliedPlanToken) -> Result<RuntimeSnapshot, &'static str> {
        if !Rc::ptr_eq(&token.owner, &self.owner) || token.generation != self.applied.generation {
            return Err("AppliedPlanExpired");
        }
        Ok(RuntimeSnapshot(self.applied.definition.clone()))
    }
    fn preview(&self) -> Definition {
        self.saved.clone()
    }
    fn apply(&mut self, preview: Definition) -> Result<(), &'static str> {
        if preview != self.saved {
            return Err("ApplyCandidateChanged");
        }
        self.applied = AppliedSnapshot {
            definition: preview,
            generation: self.applied.generation + 1,
        };
        Ok(())
    }
}
#[test]
fn saved_asset_changes_do_not_invalidate_self_contained_applied_or_runtime_snapshot() {
    let mut plans = Plans::new();
    let token_a = plans.token();
    let running = plans.start(&token_a).unwrap();
    plans.saved = Definition {
        model: "B".into(),
        asset: b"asset-B".to_vec(),
    };
    assert_eq!(plans.start(&token_a).unwrap().0, running.0);
    assert_eq!(running.0.asset, b"asset-A");
    let preview_b = plans.preview();
    plans.saved.asset = b"externally-replaced-B".to_vec();
    assert_eq!(plans.apply(preview_b), Err("ApplyCandidateChanged"));
    assert!(plans.start(&token_a).is_ok());
    plans.apply(plans.preview()).unwrap();
    assert!(plans.start(&token_a).is_err());
    assert_eq!(
        plans.start(&plans.token()).unwrap().0.asset,
        b"externally-replaced-B"
    );
    assert_eq!(running.0.model, "A");
    assert!(Plans::new().start(&plans.token()).is_err());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fc {
    St,
    Mx,
    Co,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NodeRef {
    ld: &'static str,
    ln: &'static str,
    object: &'static str,
    attribute: &'static str,
    fc: Fc,
    index: Option<usize>,
}
#[derive(Clone, Debug)]
enum Kind {
    Bool,
    Float,
    Structure,
    Array(usize),
}
struct NativeModel {
    nodes: BTreeMap<NodeRef, Kind>,
    dataset: Vec<NodeRef>,
    rcb_dataset: &'static str,
    control: NodeRef,
    feedback: NodeRef,
}
impl NativeModel {
    fn resolve(&self, r: &NodeRef) -> Result<&Kind, &'static str> {
        if let Some(kind) = self.nodes.get(r) {
            return Ok(kind);
        }
        let base = NodeRef {
            index: None,
            ..r.clone()
        };
        if let (Some(index), Some(Kind::Array(len))) = (r.index, self.nodes.get(&base)) {
            if index < *len {
                return Ok(self.nodes.get(&base).unwrap());
            }
        }
        Err("NativeReferenceInvalid")
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.rcb_dataset != "ds" {
            return Err("DataSetMissing");
        }
        for reference in &self.dataset {
            self.resolve(reference)?;
        }
        if self.control.fc != Fc::Co || self.feedback.fc != Fc::St {
            return Err("ControlTargetInvalid");
        }
        self.resolve(&self.control)?;
        self.resolve(&self.feedback)?;
        Ok(())
    }
    fn bind_scalar(&self, r: &NodeRef) -> Result<(), &'static str> {
        match self.resolve(r)? {
            Kind::Bool | Kind::Float => Ok(()),
            _ => Err("UnsupportedSignalConversion"),
        }
    }
}
fn node(attribute: &'static str, fc: Fc) -> NodeRef {
    NodeRef {
        ld: "LD0",
        ln: "GGIO1",
        object: "Ind1",
        attribute,
        fc,
        index: None,
    }
}
fn fixture() -> NativeModel {
    let st = node("stVal", Fc::St);
    let mx = node("stVal", Fc::Mx);
    let control = node("Oper", Fc::Co);
    let array = node("samples", Fc::Mx);
    NativeModel {
        nodes: BTreeMap::from([
            (st.clone(), Kind::Bool),
            (mx.clone(), Kind::Float),
            (control.clone(), Kind::Structure),
            (array.clone(), Kind::Array(3)),
            (node("q", Fc::St), Kind::Bool),
        ]),
        dataset: vec![
            mx,
            NodeRef {
                index: Some(2),
                ..array
            },
            st.clone(),
        ],
        rcb_dataset: "ds",
        control,
        feedback: st,
    }
}
#[test]
fn native_identity_preserves_fc_structure_arrays_dataset_order_and_unbound_nodes() {
    let model = fixture();
    model.validate().unwrap();
    assert!(matches!(
        model.resolve(&node("stVal", Fc::St)),
        Ok(Kind::Bool)
    ));
    assert!(matches!(
        model.resolve(&node("stVal", Fc::Mx)),
        Ok(Kind::Float)
    ));
    assert_eq!(
        model.dataset.iter().map(|r| r.fc).collect::<Vec<_>>(),
        [Fc::Mx, Fc::Mx, Fc::St]
    );
    assert!(
        model.resolve(&node("q", Fc::St)).is_ok(),
        "未绑定Signal的原生节点仍可查询"
    );
    assert_eq!(
        model.bind_scalar(&model.control),
        Err("UnsupportedSignalConversion")
    );
    assert!(model
        .resolve(&NodeRef {
            index: Some(3),
            ..node("samples", Fc::Mx)
        })
        .is_err());
    assert!(model.resolve(&node("stVal", Fc::Co)).is_err());
    assert!(model
        .resolve(&NodeRef {
            ld: "other-LD",
            ..node("stVal", Fc::St)
        })
        .is_err());
    let mut broken = fixture();
    broken.rcb_dataset = "missing";
    assert_eq!(broken.validate(), Err("DataSetMissing"));
    let mut reordered = fixture();
    reordered.dataset.reverse();
    assert_ne!(
        reordered.dataset, model.dataset,
        "DataSet成员顺序属于协议定义，不可排序成集合"
    );
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Service {
    Mms,
    Goose,
}
enum Change {
    NativeModel(&'static str),
    Mapping(Service),
}
struct Services {
    live: BTreeMap<Service, (&'static str, Rc<NativeModel>)>,
    rcb_enabled: bool,
    conf_rev: u32,
}
impl Services {
    fn affected(&self, change: Change) -> BTreeSet<Service> {
        self.live
            .iter()
            .filter_map(|(service, (model, _))| match change {
                Change::NativeModel(changed) if *model == changed => Some(*service),
                Change::Mapping(changed) if *service == changed => Some(*service),
                _ => None,
            })
            .collect()
    }
    fn stop(&mut self, service: Service) {
        self.live.remove(&service);
    }
}
#[test]
fn service_dependencies_and_runtime_plane_do_not_collapse_into_one_connection_or_saved_definition()
{
    let model = Rc::new(fixture());
    let mut services = Services {
        live: BTreeMap::from([
            (Service::Mms, ("ied-A", model.clone())),
            (Service::Goose, ("ied-A", model.clone())),
        ]),
        rcb_enabled: false,
        conf_rev: 1,
    };
    let saved_dataset = model.dataset.clone();
    services.rcb_enabled = true;
    services.conf_rev += 1;
    assert_eq!(model.dataset, saved_dataset);
    assert_eq!(
        services.affected(Change::NativeModel("ied-A")),
        BTreeSet::from([Service::Mms, Service::Goose])
    );
    assert!(services
        .affected(Change::NativeModel("unrelated-ied"))
        .is_empty());
    assert_eq!(
        services.affected(Change::Mapping(Service::Mms)),
        BTreeSet::from([Service::Mms])
    );
    assert_eq!(Rc::strong_count(&model), 3);
    services.stop(Service::Mms);
    assert_eq!(Rc::strong_count(&model), 2);
    assert!(services.live.contains_key(&Service::Goose));
    services.stop(Service::Goose);
    assert_eq!(Rc::strong_count(&model), 1);
    // 无原生库、socket或生产注册，不生成通信成功。
}
