use dioxus::prelude::*;
use pixus_macro::html;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut count = use_signal(|| 0);

    html! {
        <main class="container">
            <h1>"Pixus rstml html! POC"</h1>
            <p class="lead">"This template is parsed by rstml, then emitted as Dioxus RSX."</p>
            <p>"Count: {count}"</p>
            <button type="button" onclick={move |_| count += 1}>
                "Increment"
            </button>
            <input disabled value="rstml boolean attribute demo" />
        </main>
    }
}
