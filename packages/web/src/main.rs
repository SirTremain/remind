use dioxus::prelude::*;
#[cfg(feature = "server")]
use dioxus::server::axum::Router;

use ui::Navbar;
use views::{Blog, Home, SignIn, SignUp};

mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(WebNavbar)]
    #[route("/")]
    Home {},
    #[route("/sign-in")]
    SignIn {},
    #[route("/sign-up")]
    SignUp {},
    #[route("/blog/:id")]
    Blog { id: i32 },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(App);
}

#[cfg(feature = "server")]
fn main() {
    serve(|| async move {
        let auth_layers = api::auth::build_auth_layers().await?;
        let router = Router::new()
            .serve_dioxus_application(ServeConfig::new(), App)
            .layer(auth_layers.auth_layer)
            .layer(auth_layers.session_layer);

        Ok(router)
    });
}

#[component]
fn App() -> Element {
    // Load initial auth state during SSR so first paint matches hydrated client state.
    let initial_auth_user =
        use_server_future(|| async { api::session_user().await.ok().flatten() })?;
    let auth_user = use_signal(move || initial_auth_user().flatten());
    use_context_provider(|| auth_user);

    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        Router::<Route> {}
    }
}

/// A web-specific Router around the shared `Navbar` component
/// which allows us to use the web-specific `Route` enum.
#[component]
fn WebNavbar() -> Element {
    let navigator = use_navigator();
    let mut auth_user = use_context::<Signal<Option<api::AuthUser>>>();
    let mut profile_menu_open = use_signal(|| false);
    let mut logout_loading = use_signal(|| false);

    let profile_initial = auth_user()
        .as_ref()
        .and_then(|user| user.display_name.chars().next())
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "U".to_owned());

    rsx! {
        Navbar {
            div {
                class: "nav-left",
                Link {
                    class: "nav-link",
                    to: Route::Home {},
                    "Home"
                }
                Link {
                    class: "nav-link",
                    to: Route::Blog { id: 1 },
                    "Blog"
                }
            }

            div {
                class: "nav-auth",
                if auth_user().is_none() {
                    Link {
                        class: "sign-in-btn",
                        to: Route::SignIn {},
                        "Sign In"
                    }
                    Link {
                        class: "sign-up-btn",
                        to: Route::SignUp {},
                        "Sign Up"
                    }
                } else {
                    div {
                        class: "profile-shell",

                        button {
                            class: "profile-icon-btn",
                            onclick: move |_| profile_menu_open.with_mut(|open| *open = !*open),
                            "{profile_initial}"
                        }

                        if profile_menu_open() {
                            div {
                                class: "profile-dropdown",
                                if let Some(user) = auth_user() {
                                    p { class: "profile-name", "{user.display_name}" }
                                    p { class: "profile-email", "{user.username}" }
                                }
                                button {
                                    class: "profile-logout-btn",
                                    disabled: logout_loading(),
                                    onclick: move |_| {
                                        let navigator = navigator.clone();
                                        async move {
                                            logout_loading.set(true);
                                            let _ = api::logout().await;
                                            auth_user.set(None);
                                            profile_menu_open.set(false);
                                            logout_loading.set(false);
                                            navigator.push(Route::Home {});
                                        }
                                    },
                                    {if logout_loading() { "Logging out..." } else { "Logout" }}
                                }
                            }
                        }
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}
