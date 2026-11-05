//! The screens driver over IPC on the mock runtime with the real capabilities (KINIG-92, ADR-0016). Without the cargo feature the command
//! does not exist; with it, the app windows may answer a step they were sent, and garbage requests are answered or dropped, never run.

#![cfg(test)]

mod common;

use common::*;
use serde_json::json;

#[cfg(not(feature = "screens-driver"))]
#[test]
fn the_default_build_has_no_driver_command_for_any_window() {
    let fixture = fixture();
    for label in ["main", "settings", "capture", "other"] {
        let window = window(&fixture, label);
        for message in [
            json!({"kind": "ready"}),
            json!({"kind": "reply", "id": "r1", "ok": true}),
        ] {
            let error = call(
                &window,
                "screens_driver_reply",
                json!({ "message": message }),
            )
            .expect_err("the command must not exist");
            let text = error.to_string();
            // Neither registered nor permitted: an unknown command or a denied one, never an answer.
            assert!(
                text.contains("not found")
                    || text.contains("not allowed")
                    || text.contains("Command"),
                "{label}: {text}"
            );
        }
    }
}

#[cfg(feature = "screens-driver")]
mod with_the_driver {
    use super::*;
    use kinige_lib::screens_driver::{DriverState, STEP_EVENT, tick};
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tauri::{Listener, Manager};

    struct Driven {
        fixture: Fixture,
        dir: tempfile::TempDir,
    }

    fn driven() -> Driven {
        let fixture = fixture();
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("requests")).unwrap();
        fixture
            .app
            .manage(DriverState::new(Some(dir.path().to_path_buf())));
        Driven { fixture, dir }
    }

    impl Driven {
        fn tick(&self) {
            let state = self.fixture.app.state::<DriverState>();
            tick(self.fixture.app.handle(), &state);
        }

        fn request(&self, id: &str, text: &str) {
            fs::write(
                self.dir.path().join("requests").join(format!("{id}.json")),
                text,
            )
            .unwrap();
        }

        fn reply(&self, id: &str) -> Option<serde_json::Value> {
            let text =
                fs::read_to_string(self.dir.path().join("replies").join(format!("{id}.json")))
                    .ok()?;
            Some(serde_json::from_str(&text).unwrap())
        }

        fn request_exists(&self, id: &str) -> bool {
            self.dir
                .path()
                .join("requests")
                .join(format!("{id}.json"))
                .exists()
        }

        /// Records what the app sends to a window.
        fn steps(
            &self,
            window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        ) -> Arc<Mutex<Vec<serde_json::Value>>> {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let sink = seen.clone();
            window.listen(STEP_EVENT, move |event| {
                sink.lock()
                    .unwrap()
                    .push(serde_json::from_str(event.payload()).unwrap());
            });
            seen
        }
    }

    fn ready(window: &tauri::WebviewWindow<tauri::test::MockRuntime>) {
        call(
            window,
            "screens_driver_reply",
            json!({"message": {"kind": "ready"}}),
        )
        .unwrap();
    }

    #[test]
    fn the_app_windows_may_say_ready_and_other_windows_may_not() {
        let driven = driven();
        for label in ["main", "settings", "capture"] {
            ready(&window(&driven.fixture, label));
        }
        let stranger = window(&driven.fixture, "stranger");
        let error = call(
            &stranger,
            "screens_driver_reply",
            json!({"message": {"kind": "ready"}}),
        )
        .expect_err("a window without the capability");
        assert!(is_acl_denial(&error), "{error}");
    }

    #[test]
    fn a_step_goes_to_a_ready_window_and_its_answer_becomes_the_reply_file() {
        let driven = driven();
        let main = window(&driven.fixture, "main");
        let steps = driven.steps(&main);
        driven.request(
            "r1",
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":"Play"}}"#,
        );
        // The page is not listening yet: the request waits where it is.
        driven.tick();
        assert!(driven.request_exists("r1"));
        assert!(driven.reply("r1").is_none());
        ready(&main);
        driven.tick();
        assert!(
            !driven.request_exists("r1"),
            "the file goes before the step runs"
        );
        assert_eq!(
            steps.lock().unwrap().as_slice(),
            [json!({"id": "r1", "step": {"kind": "press", "name": "Play"}})]
        );
        driven.tick();
        assert_eq!(steps.lock().unwrap().len(), 1, "a step is sent once");
        call(
            &main,
            "screens_driver_reply",
            json!({"message": {"kind": "reply", "id": "r1", "ok": true, "value": "fine"}}),
        )
        .unwrap();
        assert_eq!(
            driven.reply("r1").unwrap(),
            json!({"id": "r1", "ok": true, "value": "fine"})
        );
    }

    #[test]
    fn a_window_answers_only_for_a_step_it_was_sent() {
        let driven = driven();
        let main = window(&driven.fixture, "main");
        let settings = window(&driven.fixture, "settings");
        ready(&main);
        driven.request(
            "r1",
            r#"{"id":"r1","window":"main","step":{"kind":"text"}}"#,
        );
        driven.tick();
        let reply = |window, id: &str| {
            call(
                window,
                "screens_driver_reply",
                json!({"message": {"kind": "reply", "id": id, "ok": true, "value": "x"}}),
            )
        };
        assert!(reply(&settings, "r1").is_err(), "another window");
        assert!(reply(&main, "never-sent").is_err(), "an id nobody was sent");
        assert!(reply(&main, "../escape").is_err(), "a path");
        assert!(reply(&main, "").is_err(), "an empty id");
        assert!(driven.reply("r1").is_none());
        assert!(reply(&main, "r1").is_ok());
        assert!(
            reply(&main, "r1").is_err(),
            "a second answer to the same step"
        );
        assert!(
            !driven
                .dir
                .path()
                .parent()
                .unwrap()
                .join("escape.json")
                .exists()
        );
    }

    #[test]
    fn an_error_answer_and_a_garbage_answer() {
        let driven = driven();
        let main = window(&driven.fixture, "main");
        ready(&main);
        driven.request(
            "r1",
            r#"{"id":"r1","window":"main","step":{"kind":"press","name":"x"}}"#,
        );
        driven.tick();
        call(&main, "screens_driver_reply", json!({"message": {"kind": "reply", "id": "r1", "ok": false, "error": "not found: \"x\""}})).unwrap();
        assert_eq!(
            driven.reply("r1").unwrap(),
            json!({"id": "r1", "ok": false, "error": "not found: \"x\""})
        );
        for garbage in [
            json!({}),
            json!({"message": null}),
            json!({"message": "ready"}),
            json!({"message": {"kind": "eval"}}),
            json!({"message": {"kind": "reply"}}),
            json!({"message": {"kind": "reply", "id": 5, "ok": true}}),
            json!({"message": {"kind": "reply", "id": "r1", "ok": "yes"}}),
        ] {
            assert!(
                call(&main, "screens_driver_reply", garbage.clone()).is_err(),
                "{garbage}"
            );
        }
    }

    #[test]
    fn front_is_done_by_the_app_and_answered_at_once() {
        let driven = driven();
        window(&driven.fixture, "settings");
        driven.request(
            "f1",
            r#"{"id":"f1","window":"settings","step":{"kind":"front"}}"#,
        );
        driven.tick();
        assert_eq!(driven.reply("f1").unwrap(), json!({"id": "f1", "ok": true}));
        assert!(!driven.request_exists("f1"));
    }

    #[test]
    fn a_window_that_does_not_exist_gets_an_error_not_a_hang() {
        let driven = driven();
        driven.request(
            "w1",
            r#"{"id":"w1","window":"settings","step":{"kind":"text"}}"#,
        );
        driven.tick();
        assert_eq!(
            driven.reply("w1").unwrap(),
            json!({"id": "w1", "ok": false, "error": "no such window"})
        );
    }

    #[test]
    fn garbage_requests_get_an_error_and_run_nothing() {
        let driven = driven();
        let main = window(&driven.fixture, "main");
        ready(&main);
        let steps = driven.steps(&main);
        let cases = [
            ("g1", "not json at all", "bad_json"),
            (
                "g2",
                r#"{"id":"g2","window":"nowhere","step":{"kind":"text"}}"#,
                "unknown_window",
            ),
            (
                "g3",
                r#"{"id":"g3","window":"main","step":{"kind":"eval","code":"alert(1)"}}"#,
                "unknown_step",
            ),
            (
                "g4",
                r#"{"id":"g4","window":"main","step":{"kind":"press"}}"#,
                "invalid_step",
            ),
            (
                "g5",
                r#"{"id":"someone-else","window":"main","step":{"kind":"text"}}"#,
                "id_mismatch",
            ),
            ("g6", "{", "bad_json"),
        ];
        for (id, text, _) in cases {
            driven.request(id, text);
        }
        // Too large: refused without being read.
        driven.request("g7", &" ".repeat(70_000));
        driven.tick();
        for (id, _, category) in cases {
            assert_eq!(
                driven.reply(id).unwrap(),
                json!({"id": id, "ok": false, "error": category}),
                "{id}"
            );
            assert!(!driven.request_exists(id));
        }
        assert_eq!(driven.reply("g7").unwrap()["error"], "too_large");
        assert!(steps.lock().unwrap().is_empty(), "nothing reached a window");
    }

    #[test]
    fn a_path_in_a_name_writes_nothing_outside_the_directory() {
        let driven = driven();
        ready(&window(&driven.fixture, "main"));
        let requests = driven.dir.path().join("requests");
        // Names that are not `<id>.json`: dropped, no reply anywhere.
        for name in ["..json", "a b.json", "x.json.bak", "é.json", "r1", "r1.txt"] {
            fs::write(
                requests.join(name),
                r#"{"id":"r1","window":"main","step":{"kind":"text"}}"#,
            )
            .unwrap();
        }
        // A link to a file elsewhere is not followed.
        #[cfg(unix)]
        {
            let outside = driven.dir.path().join("outside.json");
            fs::write(
                &outside,
                r#"{"id":"l1","window":"main","step":{"kind":"text"}}"#,
            )
            .unwrap();
            std::os::unix::fs::symlink(&outside, requests.join("l1.json")).unwrap();
        }
        // A directory with a good name is left alone.
        fs::create_dir(requests.join("d1.json")).unwrap();
        // A script's temporary file is not picked up before its rename.
        fs::write(requests.join(".t1.json.tmp"), "{").unwrap();
        driven.tick();
        let steps_after = fs::read_dir(&requests).unwrap().count();
        assert!(
            steps_after <= 4,
            "the stray files were dropped ({steps_after} left)"
        );
        assert!(requests.join("d1.json").is_dir());
        assert!(requests.join(".t1.json.tmp").exists());
        #[cfg(unix)]
        {
            assert_eq!(driven.reply("l1").unwrap()["error"], "not_a_file");
            assert!(
                driven.dir.path().join("outside.json").exists(),
                "the link's target is untouched"
            );
        }
        let replies = driven.dir.path().join("replies");
        for entry in fs::read_dir(replies).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                Path::new(&name).extension().is_some_and(|e| e == "json") && !name.starts_with('.'),
                "{name}"
            );
        }
    }

    #[test]
    fn an_idle_driver_does_nothing() {
        let fixture = fixture();
        fixture.app.manage(DriverState::new(None));
        let main = window(&fixture, "main");
        ready(&main);
        let state = fixture.app.state::<DriverState>();
        tick(fixture.app.handle(), &state);
        let error = call(
            &main,
            "screens_driver_reply",
            json!({"message": {"kind": "reply", "id": "r1", "ok": true}}),
        )
        .expect_err("idle");
        assert_eq!(error, "idle");
    }
}
