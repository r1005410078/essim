//! 适配器显式返回已提交事件，旧Receipt接口不重复执行命令。
use essim_core::{
    application::{
        commands::{Action, Command},
        events::ConfigurationFact,
    },
    domain::{identity::ModelId, model::CollectionModel},
};
use essim_infrastructure::workspace::WorkspaceEngine;
#[test]
fn event_entry_exposes_committed_fact_and_receipt_entry_commits_once() {
    let engine = WorkspaceEngine::default();
    let model = CollectionModel {
        id: ModelId("m".into()),
        name: "M".into(),
        points: vec![],
    };
    let (receipt, event) = engine
        .execute_with_event(Command {
            command_id: "cause".into(),
            action: Action::SaveModel {
                model: model.clone(),
            },
        })
        .unwrap();
    assert_eq!(
        event.fact,
        ConfigurationFact::ModelSaved {
            id: model.id.clone()
        }
    );
    assert_eq!(event.command_id, "cause");
    assert_eq!(receipt.sequence, 1);
    assert_eq!(event.catalog, engine.query().unwrap().catalog);
    let receipt = engine
        .execute(Command {
            command_id: "next".into(),
            action: Action::SaveModel { model },
        })
        .unwrap();
    assert_eq!(receipt.sequence, 2);
    assert_eq!(engine.sequence().unwrap(), 2);
    assert_eq!(engine.query().unwrap().catalog.models.len(), 1);
}
