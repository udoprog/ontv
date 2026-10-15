use auth::yew::AuthProvider;
use yew::prelude::*;

use crate::app::App;

/// Signs the user in before the app, and its websocket, start.
#[function_component]
pub(super) fn Root() -> Html {
    html! {
        <AuthProvider>
            <App />
        </AuthProvider>
    }
}
