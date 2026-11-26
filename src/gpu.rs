#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub dedicated_memory: u64,
}

/// Vulkanバックエンドで利用できるGPUを検出する。
///
/// Vulkan featureなしでビルドされた場合は常に `None` を返し、呼び出し側はCPU動作となる。
pub fn detect_gpu() -> Option<GpuInfo> {
    if !cfg!(feature = "vulkan") {
        return None;
    }
    platform::detect()
}

#[cfg(target_os = "windows")]
mod platform {
    use super::GpuInfo;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
    };

    pub fn detect() -> Option<GpuInfo> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
            let mut best: Option<GpuInfo> = None;
            let mut index = 0u32;
            while let Ok(adapter) = factory.EnumAdapters1(index) {
                index += 1;
                let Ok(desc) = adapter.GetDesc1() else {
                    continue;
                };
                if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                    continue;
                }
                let name = String::from_utf16_lossy(&desc.Description)
                    .trim_end_matches('\0')
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let memory = desc.DedicatedVideoMemory as u64;
                if best
                    .as_ref()
                    .is_none_or(|current| memory > current.dedicated_memory)
                {
                    best = Some(GpuInfo {
                        name,
                        dedicated_memory: memory,
                    });
                }
            }
            best
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::GpuInfo;

    pub fn detect() -> Option<GpuInfo> {
        None
    }
}
