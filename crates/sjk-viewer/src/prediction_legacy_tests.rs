use super::*;

#[test]
fn predict_legacy_bits_reach_the_viewer_steps_and_survive_serverinfo() {
    let mut prediction = LocalPrediction::new(None, None, None, &VirtualFileSystem::new());
    prediction.set_predict_legacy(predict_legacy::ALL | 0x100);
    assert_eq!(
        prediction.movement_config.predict_legacy,
        predict_legacy::ALL
    );
    assert!(prediction.saber_lock_view.off);
    assert!(prediction.entities.off);
    prediction.history_gap.discarded(8, 0);
    prediction.set_predict_legacy(predict_legacy::HISTORY_GAP);
    assert!(!prediction.history_gap.incomplete());
    assert!(!prediction.saber_lock_view.off);
    assert!(!prediction.entities.off);
    let game = GameState::empty_local(0);
    prediction.refresh_roll_rules(&game);
    assert_eq!(
        prediction.movement_config.predict_legacy,
        predict_legacy::HISTORY_GAP
    );
    prediction.set_predict_legacy(0);
    assert_eq!(prediction.movement_config.predict_legacy, 0);
}
