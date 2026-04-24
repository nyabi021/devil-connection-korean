fn main() {
    #[cfg(windows)]
    {
        embed_resource::compile("assets/resources.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
