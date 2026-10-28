#include "notifications_windows.h"
#include <QCoreApplication>
#include <QDateTime>
#include <QJsonDocument>
#include <QDir>
#include <algorithm>
#include <windows.h>
#include <shobjidl.h>
#include <shlobj.h>
#include <propkey.h>
#include <propvarutil.h>
namespace {
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
    if(SUCCEEDED(result))result=properties->Commit();
    if(SUCCEEDED(result))result=link->QueryInterface(IID_PPV_ARGS(&persist));
    if(SUCCEEDED(result)){const auto file=(QString::fromWCharArray(programs)+"/Luma Mail.lnk").toStdWString();result=persist->Save(file.c_str(),TRUE);}
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
    auto path=qEnvironmentVariable("LUMA_WEBVIEW");if(path.isEmpty())path=QCoreApplication::applicationDirPath()+"/luma_webview.dll";
    library.setFileName(path);library.setLoadHints(QLibrary::PreventUnloadHint);if(library.load()){show=reinterpret_cast<Show>(library.resolve("luma_notification_show"));free=reinterpret_cast<decltype(free)>(library.resolve("luma_notification_free"));}
    }
    connect(QCoreApplication::instance(),&QCoreApplication::aboutToQuit,this,&WindowsNotifications::clear);
}
WindowsNotifications::~WindowsNotifications(){clear();}
void WindowsNotifications::notify(const QVariantMap&message){
    if(!show||!free){emit status("Windows notification support is missing from this package.");return;}
    if(!identity){identity=registerIdentity();if(!identity){emit status("Windows could not register Luma's notification shortcut.");return;}}
    expire();while(entries.size()>=64){auto first=std::min_element(entries.keyBegin(),entries.keyEnd());retire(*first);}
    const auto token=++next;const auto data=QJsonDocument::fromVariant(QVariantMap{{"title",message.value("title")},{"body",message.value("body")},{"copyCode",validCode(message.value("code").toString())}}).toJson(QJsonDocument::Compact);
    auto handle=show(reinterpret_cast<const unsigned char*>(data.constData()),size_t(data.size()),callback,token);
    if(!handle){emit status("Windows notifications are unavailable. Check Luma in Windows notification settings.");return;}
    entries.insert(token,{message,handle,QDateTime::currentMSecsSinceEpoch()+600000});expire();emit status(QString());
}
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
