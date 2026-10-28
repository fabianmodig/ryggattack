use super::*;

#[test]
fn explosion_metadata_is_bounded_presentation_only_and_optional_for_v4() {
    let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
    crate::explosions::replication_test_burst(sim.app.world_mut(), 0);
    let mut frame = sim.network_frame();
    frame.bodies.retain(|body| body.kind == 4);
    assert!(valid_frame(&frame));
    for progress in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        let mut bad = frame.clone();
        bad.bodies[0].explosion.as_mut().unwrap().progress = progress;
        assert!(!valid_frame(&bad));
    }
    let central = frame
        .bodies
        .iter()
        .position(|b| b.explosion.unwrap().burst.is_some())
        .unwrap();
    for (ordinal, chain) in [(3, None), (0, Some(3)), (1, Some(0))] {
        let mut bad = frame.clone();
        let effect = bad.bodies[central].explosion.as_mut().unwrap();
        effect.ordinal = ordinal;
        effect.burst = chain;
        assert!(!valid_frame(&bad));
    }
    let mut bad = frame.clone();
    bad.bodies[central].kind = 1;
    assert!(!valid_frame(&bad));
    let mut old = serde_json::to_value(frame).unwrap();
    for body in old["bodies"].as_array_mut().unwrap() {
        body.as_object_mut().unwrap().remove("explosion");
    }
    let old: Frame = serde_json::from_value(old).unwrap();
    assert!(valid_frame(&old));
}
