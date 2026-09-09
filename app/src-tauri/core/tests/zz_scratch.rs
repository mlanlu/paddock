use paddock_core::exit::Exit;
use paddock_core::proc::child_path;
use std::path::Path;

#[test]
fn scratch_child_path_bare_name() {
    let p = child_path(Path::new("paddock"), None);
    println!("PATH = {p:?}");
    println!("first element = {:?}", p.split(':').next());
}

#[test]
fn scratch_exit_serialize() {
    println!("Ok      -> {:?}", serde_json::to_string(&Exit::Ok));
    println!("NoDocker-> {:?}", serde_json::to_string(&Exit::NoDocker));
    println!("Other   -> {:?}", serde_json::to_string(&Exit::Other(130)));
}
