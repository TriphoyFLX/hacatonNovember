use std::process::{Command, Stdio};

use air_control_core::describe_vision_line;

#[test]
fn python_protocol_is_accepted_by_rust() {
    let vision = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vision");
    let script = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
from air_control_vision.protocol import tracking_message, hello_message
print(json.dumps(hello_message(4, "test")), flush=True)
landmarks = [[0.2, 0.3, 0.0] for _ in range(21)]
hand = {"handedness": "Right", "score": 0.88, "landmarks": landmarks}
print(json.dumps(tracking_message(1, 640, 480, 2.0, 8.0, [hand], None)), flush=True)
print("this is not json", flush=True)
"#;
    let python = ["python", "python3"]
        .into_iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        })
        .expect("Python 3 must be available as `python` or `python3`");
    let output = Command::new(python)
        .arg("-c")
        .arg(script)
        .arg(vision)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("python");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(describe_vision_line(lines.next().unwrap()), "hello");
    assert_eq!(describe_vision_line(lines.next().unwrap()), "tracking");
    assert_eq!(describe_vision_line(lines.next().unwrap()), "error");
}
