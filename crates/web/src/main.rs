mod components;
mod error;

fn main() -> anyhow::Result<()> {
    wasm_logger::init(wasm_logger::Config::default());
    log::trace!("Started up");
    yew::Renderer::<components::App>::new().render();
    Ok(())
}
