fn main() {
    glib_build_tools::compile_resources(
        &["../../data"],
        "../../data/luma.gresource.xml",
        "luma.gresource",
    );
}
