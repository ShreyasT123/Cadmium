/// The common error type for Cadmium's fallible GPU, window, asset, and platform APIs.
///
/// Most public functions that can fail return `Result<T, CadmiumError>`. GPU,
/// window, and image failures are wrapped through thiserror's `#[from]`, so
/// `?` converts them automatically. [`Self::Svg`] and [`Self::Platform`] carry
/// owned messages for cases where no source error is available.
///
/// # Examples
///
/// ```rust
/// use cadmium::core::CadmiumError;
///
/// fn load() -> Result<(), CadmiumError> {
///     Err(CadmiumError::Svg("unclosed <path>".into()))
/// }
///
/// assert_eq!(
///     load().unwrap_err().to_string(),
///     "SVG error: unclosed <path>",
/// );
/// ```
#[derive(Debug, Error)]
pub enum CadmiumError {
    /// Creating a GPU surface for an OS window failed.
    ///
    /// Wraps `wgpu::CreateSurfaceError` through `#[from]`.
    #[error("Failed to create GPU surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),

    /// No compatible GPU adapter was available for the requested options.
    ///
    /// Wraps `wgpu::RequestAdapterError` through `#[from]`.
    #[error("No compatible GPU adapter found: {0}")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),

    /// Requesting a logical device from the selected adapter failed.
    ///
    /// Wraps `wgpu::RequestDeviceError` through `#[from]`.
    #[error("Failed to request GPU device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),

    /// The `winit` event loop could not be created or run.
    ///
    /// Wraps `winit::error::EventLoopError` through `#[from]`.
    #[error("OS window event loop error: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),

    /// The operating system refused to create a window.
    ///
    /// Wraps `winit::error::OsError` through `#[from]`.
    #[error("Failed to create OS window: {0}")]
    CreateWindow(#[from] winit::error::OsError),

    /// An image file could not be decoded into pixel data.
    ///
    /// Wraps `image::ImageError` through `#[from]`.
    #[error("Image decode error: {0}")]
    Image(#[from] image::ImageError),

    /// An SVG could not be parsed or rasterized.
    ///
    /// Carries an owned message; there is no wrapped source error. This is used
    /// by SVG rasterization when parsing fails or a pixmap cannot be allocated.
    #[error("SVG error: {0}")]
    Svg(String),

    /// A native platform integration could not complete an operation.
    ///
    /// Carries an owned message; there is no wrapped source error. Examples
    /// include clipboard and file-dialog failures and an unavailable event-loop
    /// proxy during window creation.
    #[error("Platform integration error: {0}")]
    Platform(String),
}

