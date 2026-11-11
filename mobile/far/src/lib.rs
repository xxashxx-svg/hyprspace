//! The phone's way to a computer from any network. `Far.port(key)` opens a port on the phone that
//! leads to the computer with that iroh key; the app connects there with the same pinned TLS it
//! uses on Wi-Fi, so nothing past the port is new.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jint;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

/// How long finding the computer may take before the app's own timeout gives up first.
const FIND: Duration = Duration::from_secs(15);

static RUNTIME: LazyLock<Option<Runtime>> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .ok()
});

static PORTS: LazyLock<Mutex<HashMap<String, u16>>> = LazyLock::new(Mutex::default);

/// The local port for `key`, opened the first time it's asked for. 0 when it couldn't be.
fn port(key: String) -> u16 {
    let Some(rt) = RUNTIME.as_ref() else {
        return 0;
    };
    let mut ports = PORTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = ports.get(&key) {
        return *p;
    }
    let Ok(listener) = rt.block_on(TcpListener::bind(("127.0.0.1", 0))) else {
        return 0;
    };
    let Ok(port) = listener.local_addr().map(|a| a.port()) else {
        return 0;
    };
    ports.insert(key.clone(), port);
    rt.spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let key = key.clone();
            tokio::spawn(async move {
                let _ = tcp.set_nodelay(true);
                if let Ok(Some(mut far)) =
                    tokio::time::timeout(FIND, hyprspace_far::dial(&key)).await
                {
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut far).await;
                }
            });
        }
    });
    port
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_hyprspace_android_net_Far_port(
    mut env: JNIEnv,
    _: JClass,
    key: JString,
) -> jint {
    let Ok(key) = env.get_string(&key) else {
        return 0;
    };
    port(key.into()) as jint
}
