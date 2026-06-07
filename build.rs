fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/rndwalker.ico");
        resource
            .compile()
            .expect("failed to embed Windows application icon");
    }
}
