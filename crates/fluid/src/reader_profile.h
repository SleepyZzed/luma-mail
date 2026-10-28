#pragma once
#ifdef LUMA_WEBENGINE
#include <QQuickWebEngineProfile>
#include <QWebEngineUrlRequestInterceptor>
#include <QWebEngineUrlRequestInfo>
#include <QPointer>
#include <QTimer>
class InertMailRequests:public QWebEngineUrlRequestInterceptor {
public:
    explicit InertMailRequests(QObject*owner):QWebEngineUrlRequestInterceptor(owner){}
    void interceptRequest(QWebEngineUrlRequestInfo&info)override{auto scheme=info.requestUrl().scheme();if(scheme!="data"&&scheme!="about")info.block(true);}
};
QQuickWebEngineProfile*createReaderProfile(QObject*owner);
class ReaderProfiles:public QObject {
    Q_OBJECT
public:
    explicit ReaderProfiles(QObject*owner=nullptr);
    Q_INVOKABLE QQuickWebEngineProfile*acquire();
    Q_INVOKABLE void release();
private:
    QPointer<QQuickWebEngineProfile>profile;
    QTimer idleRelease;
    int viewers=0;
};
#endif
