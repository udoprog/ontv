use auth::yew023::AuthProvider;
use phrasebook::Phrases;
use yew::prelude::*;

use crate::app::App;
use crate::l10n;

/// Signs the user in before the app, and its websocket, start.
#[function_component]
pub(super) fn Root() -> Html {
    let phrases = use_memo((), |()| l10n::initial());

    html! {
        <ContextProvider<Phrases> context={(*phrases).clone()}>
            <AuthProvider>
                <App />
            </AuthProvider>
        </ContextProvider<Phrases>>
    }
}
