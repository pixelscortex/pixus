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
            <h1>"Pixus html! playground"</h1>
            <p class="lead">"HTML-like syntax lowered into normal Dioxus RSX."</p>
            <p>"Count: {count}"</p>
            <button type="button" onclick={move |_| count += 1}>
                "Increment"
            </button>
            <input disabled value="rstml boolean attribute demo" />
        </main>
    }
}
