#include "startup_graphics.h"
#include <QGuiApplication>
#include <QQuickWindow>
#include <QSGRendererInterface>
#ifdef Q_OS_LINUX
#include <QOpenGLContext>
#include <QOffscreenSurface>
#if QT_CONFIG(vulkan)
#include <QVulkanInstance>
#include <QVulkanFunctions>
#include <vector>
#include <cstring>
#endif
#endif
QString selectStartupGraphics(){
    // Respect explicit troubleshooting choices. CI/X11 also retain their normal
    // renderer. Probe once before creating any Quick window; no idle polling.
    if(!qEnvironmentVariableIsEmpty("QSG_RHI_BACKEND")||!qEnvironmentVariableIsEmpty("QT_QUICK_BACKEND"))return "explicit";
#ifdef Q_OS_LINUX
    if(!QGuiApplication::platformName().startsWith("wayland"))return "platform-default";
#if QT_CONFIG(vulkan)
    QVulkanInstance vulkan;vulkan.setApiVersion(QVersionNumber(1,1));
    if(vulkan.create()){
        auto functions=vulkan.functions();uint32_t count=0;
        if(functions->vkEnumeratePhysicalDevices(vulkan.vkInstance(),&count,nullptr)==VK_SUCCESS&&count>0&&count<=64){
            std::vector<VkPhysicalDevice>devices(count);
            if(functions->vkEnumeratePhysicalDevices(vulkan.vkInstance(),&count,devices.data())==VK_SUCCESS)for(auto device:devices){
                VkPhysicalDeviceProperties properties{};functions->vkGetPhysicalDeviceProperties(device,&properties);
                // Do not prefer a CPU Vulkan implementation over hardware GL.
                if(properties.deviceType==VK_PHYSICAL_DEVICE_TYPE_CPU||properties.apiVersion<VK_API_VERSION_1_1)continue;
                uint32_t families=0;functions->vkGetPhysicalDeviceQueueFamilyProperties(device,&families,nullptr);if(!families||families>256)continue;
                std::vector<VkQueueFamilyProperties>queues(families);functions->vkGetPhysicalDeviceQueueFamilyProperties(device,&families,queues.data());
                bool graphics=false;for(const auto&queue:queues)graphics|=queue.queueCount>0&&(queue.queueFlags&VK_QUEUE_GRAPHICS_BIT);
                uint32_t extensions=0;if(!graphics||functions->vkEnumerateDeviceExtensionProperties(device,nullptr,&extensions,nullptr)!=VK_SUCCESS||extensions>4096)continue;
                std::vector<VkExtensionProperties>supported(extensions);bool swapchain=false;
                if(functions->vkEnumerateDeviceExtensionProperties(device,nullptr,&extensions,supported.data())==VK_SUCCESS)for(const auto&extension:supported)swapchain|=!std::strcmp(extension.extensionName,"VK_KHR_swapchain");
                if(swapchain){QQuickWindow::setGraphicsApi(QSGRendererInterface::Vulkan);return "vulkan";}
            }
        }
    }
#endif
    QOpenGLContext context;QOffscreenSurface surface;surface.create();
    if(context.create()&&surface.isValid()&&context.makeCurrent(&surface)){context.doneCurrent();QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);return "opengl";}
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);return "software-fallback";
#else
    return "platform-default";
#endif
}
