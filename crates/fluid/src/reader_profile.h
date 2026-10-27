#pragma once
#ifdef LUMA_WEBENGINE
#include <QQuickWebEngineProfile>
#include <QWebEngineUrlRequestInterceptor>
#include <QWebEngineUrlRequestInfo>
class InertMailRequests:public QWebEngineUrlRequestInterceptor {
public:
    explicit InertMailRequests(QObject*owner):QWebEngineUrlRequestInterceptor(owner){}
    void interceptRequest(QWebEngineUrlRequestInfo&info)override{auto scheme=info.requestUrl().scheme();if(scheme!="data"&&scheme!="about")info.block(true);}
};
QQuickWebEngineProfile*createReaderProfile(QObject*owner);
#endif
