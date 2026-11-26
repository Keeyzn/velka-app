fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/velka.ico");
        res.set("ProductName", "Velka");
        res.set("FileDescription", "Velka");
        // Version des propriétés du fichier = version du paquet (sinon Windows affiche une ancienne valeur).
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        res.set("FileVersion", &version);
        res.set("ProductVersion", &version);
        if let Err(e) = res.compile() {
            println!("cargo:warning=icône non intégrée : {e}");
        }
    }
}
