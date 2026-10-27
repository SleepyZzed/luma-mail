#include "reader_profile.h"
#ifdef LUMA_WEBENGINE
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
