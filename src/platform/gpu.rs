//! Choosing the wgpu backend before the renderer starts.

/// The variable through which wgpu accepts a restriction of its backends.
///
/// It is the only lever available: iced 0.14 cannot pick the backend.
/// `iced_wgpu` reads the chosen adapter but lets wgpu decide on its own, through
/// `Backends::from_env_or_default()`.
#[cfg(target_os = "linux")]
const WGPU_BACKEND: &str = "WGPU_BACKEND";

/// Restrict wgpu to Vulkan, but only when Vulkan answers.
///
/// Unrestricted, wgpu instantiates **every** backend to enumerate their
/// adapters, then picks one. On the development machine it already
/// picked Vulkan, and even rejected the OpenGL adapter as "not compatible with
/// surface". But having probed it leaves `libnvidia-eglcore` and Mesa's
/// `libLLVM` loaded for the lifetime of the program: 39 MB resident for an
/// enumeration whose result was thrown away.
///
/// The restriction is only set after checking that a Vulkan adapter exists,
/// and that is the whole point of the function. Setting `WGPU_BACKEND=vulkan`
/// blindly on a machine with no Vulkan driver (the loader package is not always
/// installed) would leave wgpu without any adapter, and Xion would not start
/// at all. The OpenGL fallback must stay possible.
///
/// Linux only: on Windows the default backend is DX12, which nothing says should
/// be dropped for Vulkan, and macOS only has Metal.
///
/// Call it before anything starts a thread: tokio, zbus and winit come later.
#[cfg(target_os = "linux")]
pub fn prefer_vulkan_when_available() {
    // An explicit choice wins: whoever sets the variable wants what they wrote.
    if std::env::var_os(WGPU_BACKEND).is_some() {
        return;
    }

    let vulkan_answers = {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        !instance
            .enumerate_adapters(wgpu::Backends::VULKAN)
            .is_empty()
        // The instance dies here, before the variable is set.
    };

    if !vulkan_answers {
        tracing::debug!("No Vulkan adapter: wgpu chooses on its own");
        return;
    }

    // SAFETY: `set_var` requires that no other thread reads the environment.
    // This runs at the very start of `main`: neither tokio, zbus nor winit exist
    // yet, and the probing instance above was destroyed before this line.
    unsafe { std::env::set_var(WGPU_BACKEND, "vulkan") };
    tracing::info!("Vulkan adapter found: rendering restricted to Vulkan");
}

/// Elsewhere, wgpu decides on its own.
#[cfg(not(target_os = "linux"))]
pub fn prefer_vulkan_when_available() {}
