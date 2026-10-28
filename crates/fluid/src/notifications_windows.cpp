#include "notifications_windows.h"
#include <QCoreApplication>
#include <QDateTime>
#include <QJsonDocument>
#include <QDir>
#include <QUuid>
#include <algorithm>
#include <atomic>
#include <windows.h>
#include <shobjidl.h>
#include <shlobj.h>
#include <propkey.h>
#include <propvarutil.h>
#include <notificationactivationcallback.h>
namespace {
const CLSID ActivatorId={0xaad94b28,0xe682,0x4f13,{0xa0,0x26,0xda,0x0e,0x28,0x67,0x38,0xf2}};
void activation(const QString&args){if(auto app=QCoreApplication::instance())QMetaObject::invokeMethod(app,[app,args]{if(auto owner=app->findChild<WindowsNotifications*>("lumaSystemNotifications"))owner->activateArguments(args);},Qt::QueuedConnection);}
class Activator final:public INotificationActivationCallback {
    std::atomic<ULONG> refs{1};
public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id,void**out)override{if(!out)return E_POINTER;*out=nullptr;if(id==IID_IUnknown||id==__uuidof(INotificationActivationCallback)){*out=static_cast<INotificationActivationCallback*>(this);AddRef();return S_OK;}return E_NOINTERFACE;}
    ULONG STDMETHODCALLTYPE AddRef()override{return ++refs;}
    ULONG STDMETHODCALLTYPE Release()override{const auto left=--refs;if(!left)delete this;return left;}
    HRESULT STDMETHODCALLTYPE Activate(LPCWSTR app,LPCWSTR args,const NOTIFICATION_USER_INPUT_DATA*,ULONG)override{if(!app||!args||wcscmp(app,L"io.github.lumamail.Luma")||wcslen(args)>80)return E_INVALIDARG;activation(QString::fromWCharArray(args));return S_OK;}
};
class Factory final:public IClassFactory {
    std::atomic<ULONG>refs{1};
public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id,void**out)override{if(!out)return E_POINTER;*out=nullptr;if(id==IID_IUnknown||id==IID_IClassFactory){*out=static_cast<IClassFactory*>(this);AddRef();return S_OK;}return E_NOINTERFACE;}
    ULONG STDMETHODCALLTYPE AddRef()override{return ++refs;}
    ULONG STDMETHODCALLTYPE Release()override{const auto left=--refs;if(!left)delete this;return left;}
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown*outer,REFIID id,void**out)override{if(outer)return CLASS_E_NOAGGREGATION;auto value=new Activator;const auto result=value->QueryInterface(id,out);value->Release();return result;}
    HRESULT STDMETHODCALLTYPE LockServer(BOOL)override{return S_OK;}
};
bool installIdentity(){
    // Portable apps need a per-user Start Menu identity for native Windows
    // notifications. This shortcut contains only the executable path/AUMID.
    const auto initialized=CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);
    PWSTR programs=nullptr;IShellLinkW*link=nullptr;IPropertyStore*properties=nullptr;IPersistFile*persist=nullptr;
    auto result=SHGetKnownFolderPath(FOLDERID_Programs,0,nullptr,&programs);
    if(SUCCEEDED(result))result=CoCreateInstance(CLSID_ShellLink,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&link));
    const auto executable=QDir::toNativeSeparators(QCoreApplication::applicationFilePath()).toStdWString();
    if(SUCCEEDED(result))result=link->SetPath(executable.c_str());
    if(SUCCEEDED(result))result=link->SetDescription(L"Luma Mail — online-first email");
    if(SUCCEEDED(result))result=link->QueryInterface(IID_PPV_ARGS(&properties));
    PROPVARIANT id;PropVariantInit(&id);
    if(SUCCEEDED(result))result=InitPropVariantFromString(L"io.github.lumamail.Luma",&id);
    if(SUCCEEDED(result))result=properties->SetValue(PKEY_AppUserModel_ID,id);
    PropVariantClear(&id);
    if(SUCCEEDED(result))result=InitPropVariantFromCLSID(ActivatorId,&id);
    if(SUCCEEDED(result))result=properties->SetValue(PKEY_AppUserModel_ToastActivatorCLSID,id);
    if(SUCCEEDED(result))result=properties->Commit();
    if(SUCCEEDED(result))result=link->QueryInterface(IID_PPV_ARGS(&persist));
    if(SUCCEEDED(result)){const auto file=(QString::fromWCharArray(programs)+"/Luma Mail.lnk").toStdWString();result=persist->Save(file.c_str(),TRUE);}
    if(SUCCEEDED(result)){HKEY key=nullptr;auto error=RegCreateKeyExW(HKEY_CURRENT_USER,L"Software\\Classes\\CLSID\\{AAD94B28-E682-4F13-A026-DA0E286738F2}\\LocalServer32",0,nullptr,0,KEY_SET_VALUE,nullptr,&key,nullptr);if(error==ERROR_SUCCESS){const auto command=L"\""+executable+L"\" --toast-activated";error=RegSetValueExW(key,nullptr,0,REG_SZ,reinterpret_cast<const BYTE*>(command.c_str()),DWORD((command.size()+1)*sizeof(wchar_t)));RegCloseKey(key);}if(error!=ERROR_SUCCESS)result=HRESULT_FROM_WIN32(error);}
    PropVariantClear(&id);if(persist)persist->Release();if(properties)properties->Release();if(link)link->Release();if(programs)CoTaskMemFree(programs);if(SUCCEEDED(initialized))CoUninitialize();
    if(SUCCEEDED(result))SetCurrentProcessExplicitAppUserModelID(L"io.github.lumamail.Luma");
    return SUCCEEDED(result);
}
void callback(quintptr token,const unsigned char*bytes,size_t length){
    if(!bytes||length>16)return;const auto action=QString::fromUtf8(reinterpret_cast<const char*>(bytes),qsizetype(length));
    if(auto app=QCoreApplication::instance())QMetaObject::invokeMethod(app,[app,token,action]{if(auto owner=app->findChild<WindowsNotifications*>("lumaSystemNotifications"))owner->activate(token,action);},Qt::QueuedConnection);
}
bool validCode(const QString&code){return code.size()>=4&&code.size()<=8&&std::all_of(code.begin(),code.end(),[](QChar c){return c>='0'&&c<='9';});}
}
WindowsNotifications::WindowsNotifications(QObject*parent):WindowsNotifications(parent,Transport{}){}
WindowsNotifications::WindowsNotifications(QObject*parent,Transport transport):QObject(parent){
    setObjectName("lumaSystemNotifications");expiry.setSingleShot(true);connect(&expiry,&QTimer::timeout,this,&WindowsNotifications::expire);
    registerIdentity=transport.registerIdentity?transport.registerIdentity:installIdentity;
    if(transport.show){show=transport.show;free=transport.release;}
    else{
    nativeTransport=true;
    auto path=qEnvironmentVariable("LUMA_WEBVIEW");if(path.isEmpty())path=QCoreApplication::applicationDirPath()+"/luma_webview.dll";
    library.setFileName(path);library.setLoadHints(QLibrary::PreventUnloadHint);if(library.load()){show=reinterpret_cast<Show>(library.resolve("luma_notification_show"));free=reinterpret_cast<decltype(free)>(library.resolve("luma_notification_free"));}
    }
    connect(QCoreApplication::instance(),&QCoreApplication::aboutToQuit,this,&WindowsNotifications::clear);
}
WindowsNotifications::~WindowsNotifications(){clear();if(activatorCookie)CoRevokeClassObject(activatorCookie);if(apartmentOwned)CoUninitialize();}
void WindowsNotifications::notify(const QVariantMap&message){
    if(!show||!free){emit status("Windows notification support is missing from this package.");return;}
    if(nativeTransport&&!activatorCookie){if(!apartmentOwned)apartmentOwned=SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED));auto factory=new Factory;CoRegisterClassObject(ActivatorId,factory,CLSCTX_LOCAL_SERVER,REGCLS_MULTIPLEUSE,&activatorCookie);factory->Release();}
    if(nativeTransport&&!activatorCookie){emit status("Windows could not register notification action handling.");return;}
    if(!identity){identity=registerIdentity();if(!identity){emit status("Windows could not register Luma's notification shortcut.");return;}}
    expire();while(entries.size()>=64){auto first=std::min_element(entries.keyBegin(),entries.keyEnd());retire(*first);}
    const auto token=++next;const auto nonce=QUuid::createUuid().toString(QUuid::WithoutBraces);const auto data=QJsonDocument::fromVariant(QVariantMap{{"title",message.value("title")},{"body",message.value("body")},{"activation",nonce},{"copyCode",validCode(message.value("code").toString())}}).toJson(QJsonDocument::Compact);
    auto handle=show(reinterpret_cast<const unsigned char*>(data.constData()),size_t(data.size()),callback,token);
    if(!handle){emit status("Windows notifications are unavailable. Check Luma in Windows notification settings.");return;}
    entries.insert(token,{message,handle,QDateTime::currentMSecsSinceEpoch()+600000,nonce});expire();emit status(QString());
}
void WindowsNotifications::activateArguments(const QString&arguments){const auto at=arguments.indexOf(':');if(at<0)return;const auto action=arguments.left(at),nonce=arguments.mid(at+1);if(action!="open"&&action!="copy")return;if(nonce.size()!=36||QUuid(nonce).isNull())return;for(auto i=entries.cbegin();i!=entries.cend();++i)if(i->nonce==nonce){activate(i.key(),action);return;}}
void WindowsNotifications::activate(quint64 token,const QString&action){
    if(!entries.contains(token))return;const auto entry=entries.value(token);
    if(entry.expires<=QDateTime::currentMSecsSinceEpoch()){retire(token);return;}
    if(action=="open")emit activated(entry.message.value("account").toString(),entry.message.value("thread").toString());
    else if(action=="copy"&&validCode(entry.message.value("code").toString()))emit copyCode(entry.message.value("code").toString());
    else if(action=="failed")emit status("Windows could not display this notification. Check system notification settings.");
    retire(token);
}
void WindowsNotifications::retire(quint64 token){const auto entry=entries.take(token);if(entry.handle&&free)free(entry.handle);}
void WindowsNotifications::expire(){const auto now=QDateTime::currentMSecsSinceEpoch();qint64 deadline=0;for(auto token:entries.keys()){const auto time=entries.value(token).expires;if(time<=now)retire(token);else if(!deadline||time<deadline)deadline=time;}if(deadline)expiry.start(int(qMax<qint64>(1,deadline-now)));else expiry.stop();}
void WindowsNotifications::clear(){expiry.stop();for(auto token:entries.keys())retire(token);}
