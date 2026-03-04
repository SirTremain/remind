use crate::Route;
use dioxus::prelude::*;

const AUTH_CSS: Asset = asset!("/assets/auth.css");

#[component]
pub fn SignUp() -> Element {
    let navigator = use_navigator();
    let redirect_navigator = navigator.clone();
    let mut auth_user = use_context::<Signal<Option<api::AuthUser>>>();
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut error_message = use_signal(String::new);

    use_effect(move || {
        if auth_user().is_some() {
            redirect_navigator.replace(Route::Home {});
        }
    });

    rsx! {
        document::Link { rel: "stylesheet", href: AUTH_CSS }

        div {
            class: "auth-page",
            div {
                class: "auth-card",
                h1 { "Create your account" }

                form {
                    class: "auth-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        let signup_email = email().trim().to_lowercase();
                        let signup_password = password();
                        let navigator = navigator.clone();

                        async move {
                            loading.set(true);
                            error_message.set(String::new());

                            match api::create_account(signup_email, signup_password).await {
                                Ok(user) => {
                                    auth_user.set(Some(user));
                                    password.set(String::new());
                                    navigator.push(Route::Home {});
                                }
                                Err(err) => {
                                    error_message.set(err.to_string());
                                }
                            }

                            loading.set(false);
                        }
                    },
                    input {
                        class: "auth-input",
                        r#type: "email",
                        placeholder: "Email",
                        value: email,
                        oninput: move |event| email.set(event.value()),
                    }

                    input {
                        class: "auth-input",
                        r#type: "password",
                        placeholder: "Password",
                        value: password,
                        oninput: move |event| password.set(event.value()),
                    }

                    button {
                        class: "auth-submit",
                        r#type: "submit",
                        disabled: loading(),
                        {if loading() { "Creating..." } else { "Sign Up" }}
                    }
                }

                div {
                    class: "auth-divider",
                    span { "or" }
                }

                div {
                    class: "social-list",
                    button { class: "social-btn", "Continue with GitHub (template)" }
                    button { class: "social-btn", "Continue with Google (template)" }
                    button { class: "social-btn", "Continue with Apple (template)" }
                }

                p {
                    class: "auth-footer",
                    "Already have an account? "
                    Link {
                        to: Route::SignIn {},
                        "Sign in"
                    }
                }

                if !error_message().is_empty() {
                    p { class: "auth-error", "{error_message}" }
                }
            }
        }
    }
}
