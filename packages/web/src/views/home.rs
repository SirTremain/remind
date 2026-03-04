use dioxus::prelude::*;
use ui::{Echo, Hero};

#[component]
pub fn Home() -> Element {
    let auth_user = use_context::<Signal<Option<api::AuthUser>>>();

    rsx! {
        if let Some(user) = auth_user() {
            div {
                class: "home-profile",
                h1 { "Your Home" }
                p { class: "home-profile-label", "Logged in as" }
                p { class: "home-profile-value", "{user.display_name}" }
                p { class: "home-profile-subtle", "{user.username}" }
                p { class: "home-profile-subtle", "User ID: {user.id}" }
            }
        } else {
            Hero {}
            Echo {}
        }
    }
}
