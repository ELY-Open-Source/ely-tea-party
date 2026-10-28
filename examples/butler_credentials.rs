//! Run write, read, replace, read-replaced, remove, missing in separate processes with one ELY_TEST_CONFIG_DIR.

use std::time::{Duration, Instant};

use ely_tea_party::butler::Credentials;

fn main() {
    if !cfg!(debug_assertions) {
        panic!("use a debug build");
    }
    let folder = std::env::var_os("ELY_TEST_CONFIG_DIR").expect("isolated test config directory");
    assert!(std::path::Path::new(&folder).is_absolute());
    let mode = std::env::args().nth(1).expect("test mode");
    gpui_platform::headless().run(move |cx| {
        Credentials::init(cx);
        cx.spawn(async move |cx| {
            let deadline = Instant::now() + Duration::from_secs(20);
            while !cx
                .read_global::<Credentials, _>(|c, _| c.loaded("deepseek") && c.loaded("openai"))
            {
                assert!(Instant::now() < deadline, "credential read timed out");
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
            }
            match mode.as_str() {
                "write" | "replace" => {
                    let suffix = if mode == "write" {
                        "first"
                    } else {
                        "replacement"
                    };
                    for provider in ["deepseek", "openai"] {
                        cx.update(|cx| {
                            Credentials::save(
                                provider,
                                Some(format!("synthetic-{provider}-{suffix}")),
                                cx,
                            )
                        })
                        .await
                        .expect("system credential write");
                    }
                }
                "read" | "read-replaced" => {
                    let suffix = if mode == "read" {
                        "first"
                    } else {
                        "replacement"
                    };
                    cx.read_global::<Credentials, _>(|c, _| {
                        for provider in ["deepseek", "openai"] {
                            assert!(
                                c.key(provider).expect("system credential read")
                                    == Some(format!("synthetic-{provider}-{suffix}").as_str()),
                                "saved key survives a fresh process"
                            );
                        }
                    });
                }
                "remove" => {
                    for provider in ["deepseek", "openai"] {
                        cx.update(|cx| Credentials::save(provider, None, cx))
                            .await
                            .expect("system credential removal");
                    }
                }
                "missing" => cx.read_global::<Credentials, _>(|c, _| {
                    for provider in ["deepseek", "openai"] {
                        assert!(c.key(provider).expect("system credential read").is_none());
                    }
                }),
                _ => panic!("unknown test mode"),
            }
            println!("butler credentials: {mode} passed");
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
