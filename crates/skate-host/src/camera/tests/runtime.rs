use super::*;

#[test]
#[ignore = "requires the user's converted stock camera and graph assets"]
fn private_stock_camera_loads_every_graph_operation_and_shot_dependency() {
    let root = std::env::var_os("SKATE3_ASSET_ROOT").expect("set SKATE3_ASSET_ROOT");
    let root = std::path::Path::new(&root);
    let camera = CameraRuntime::load(root).expect("complete normal stock camera assets");
    assert!(camera.frame.is_none());
}
