use ktlsp_protocol::{read_frame, write_frame};
use serde_json::Value;
use std::{
    io::{BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Client {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
}
impl Client {
    pub fn new(executable: impl AsRef<Path>) -> Self {
        let mut child = Command::new(executable.as_ref())
            .arg("--stdio")
            .env("PATH", "")
            .env_remove("JAVA_HOME")
            .env_remove("JDK_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let (sender, replies) = mpsc::sync_channel(16);
        let reader = thread::spawn(move || {
            while let Some(payload) = read_frame(&mut output).unwrap() {
                if sender
                    .send(serde_json::from_slice(&payload).unwrap())
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            replies,
            reader: Some(reader),
        }
    }
    pub fn send(&mut self, message: Value) {
        let input = self.input.as_mut().unwrap();
        write_frame(input, &serde_json::to_vec(&message).unwrap()).unwrap();
        input.flush().unwrap();
    }
    pub fn receive(&self) -> Value {
        self.replies
            .recv_timeout(Duration::from_secs(3))
            .expect("native stdio reply timeout")
    }
    pub fn finish(self) {
        let errors = self.finish_status(0);
        assert!(errors.is_empty(), "{errors}");
    }
    pub fn finish_status(mut self, expected_code: i32) -> String {
        self.input.take();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(
                    status.code(),
                    Some(expected_code),
                    "native stdio exited with {status}"
                );
                break;
            }
            assert!(Instant::now() < deadline, "native stdio exit timeout");
            thread::sleep(Duration::from_millis(2));
        }
        self.reader.take().unwrap().join().unwrap();
        assert!(
            self.replies.try_recv().is_err(),
            "unexpected late stdio response"
        );
        let mut errors = String::new();
        self.child
            .stderr
            .as_mut()
            .unwrap()
            .read_to_string(&mut errors)
            .unwrap();
        errors
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
