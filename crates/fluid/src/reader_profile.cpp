#include "reader_profile.h"
#ifdef LUMA_WEBENGINE
#include <QQmlEngine>
ReaderProfiles::ReaderProfiles(QObject*owner):QObject(owner),idleRelease(this){
    idleRelease.setSingleShot(true);idleRelease.setInterval(1000);
    connect(&idleRelease,&QTimer::timeout,this,[this]{if(viewers==0&&profile){auto retired=profile.data();profile=nullptr;retired->deleteLater();}});
}
QQuickWebEngineProfile*ReaderProfiles::acquire(){
    idleRelease.stop();if(!profile){profile=createReaderProfile(this);QQmlEngine::setObjectOwnership(profile,QQmlEngine::CppOwnership);}++viewers;return profile;
}
void ReaderProfiles::release(){if(viewers>0)--viewers;if(viewers==0)idleRelease.start();}
QQuickWebEngineProfile*createReaderProfile(QObject*owner){
    auto profile=new QQuickWebEngineProfile(owner); // no name: off the record
    profile->setOffTheRecord(true);profile->setHttpCacheType(QQuickWebEngineProfile::NoCache);
    profile->setPersistentCookiesPolicy(QQuickWebEngineProfile::NoPersistentCookies);
    profile->setPersistentPermissionsPolicy(QQuickWebEngineProfile::PersistentPermissionsPolicy::StoreInMemory);
    profile->setSpellCheckEnabled(false);profile->setUrlRequestInterceptor(new InertMailRequests(profile));
    profile->setPushServiceEnabled(false);
    return profile;
}
#endif
