// SPDX-License-Identifier: AGPL-3.0-or-later

//! vhost http server to serve local content.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::Request as HttpRequest;
use axum::routing::get;
use log::error;
use parking_lot::Mutex;
use servo::prefs::{PreferencesObserver, get_embedder_pref};
use servo::{PrefValue, prefs};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeFile;
use tower_http::services::fs::ServeFileSystemResponseBody;
use url::Host::Domain;
use url::Url;

use crate::resources::resources_dir_path;

fn vhost_handler(state: ServerState, request: Request) -> Option<PathBuf> {
    let Some(host) = request.headers().get("Host") else {
        println!("No Host header!");
        return None;
    };

    let Ok(url) = format!("http://{}", host.to_str().unwrap_or_default()).parse::<Url>() else {
        println!(
            "Failed to parse http://{}",
            host.to_str().unwrap_or_default()
        );
        return None;
    };

    let Some(Domain(domain)) = url.host() else {
        return None;
    };

    let parts: Vec<String> = domain.split(".").map(|s| s.to_owned()).collect();

    if parts.len() != 2 || parts[1] != "localhost" {
        return None;
    }

    let app = &parts[0];

    // Currently recognized: system, shared, keyboard, homescreen, theme
    // TODO: don't hardcode
    if app != "homescreen" &&
        app != "keyboard" &&
        app != "settings" &&
        app != "shared" &&
        app != "system" &&
        app != "theme"
    {
        return None;
    }

    let uri_parts: Vec<String> = request
        .uri()
        .path()
        .split("/")
        .filter_map(|s| {
            if s.is_empty() || s.contains("..") {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .collect();

    if app == "theme" {
        let theme = state.theme.lock();
        Some(
            resources_dir_path()
                .join("browserhtml")
                .join("themes")
                .join(&*theme)
                .join(uri_parts.join("/")),
        )
    } else {
        Some(
            resources_dir_path()
                .join("browserhtml")
                .join(app)
                .join(uri_parts.join("/")),
        )
    }
}

#[derive(Clone)]
struct ServerState {
    theme: Arc<Mutex<String>>,
}

impl PreferencesObserver for ServerState {
    fn prefs_changed(&self, changes: &[(&'static str, PrefValue)]) {
        for (name, value) in changes {
            if *name == "browserhtml.theme" {
                let PrefValue::Str(theme) = value else {
                    error!("Unexpected value for browserhtml.theme: {value:?}");
                    return;
                };
                *self.theme.lock() = theme.to_string();
            }
        }
    }
}

async fn get_file(
    State(state): State<ServerState>,
    request: Request,
) -> impl axum::response::IntoResponse {
    let path = vhost_handler(state, request).unwrap_or_default();
    let service = ServeFile::new(path);
    <ServeFile as tower::ServiceExt<HttpRequest<ServeFileSystemResponseBody>>>::oneshot(
        service,
        HttpRequest::default(),
    )
    .await
}

pub(crate) fn start_vhost(port: u16) {
    // Get the initial value from the embedder preferences.
    let theme = match get_embedder_pref("browserhtml.theme") {
        Some(PrefValue::Str(value)) => value,
        _ => "default".to_owned(),
    };
    let state = ServerState {
        theme: Arc::new(Mutex::new(theme)),
    };

    prefs::add_observer(Box::new(state.clone()));

    // Configure CORS to allow cross-origin requests between localhost subdomains
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/{*key}", get(get_file))
        .layer(cors)
        .with_state(state);

    servo::Servo::spawn_task(async move {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .expect("Failed to bind on 127.0.0.1:{port}");
        let _ = axum::serve(listener, app).await;
    });
}
