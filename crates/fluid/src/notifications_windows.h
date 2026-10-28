#pragma once
#include <QObject>
#include <QLibrary>
#include <QHash>
#include <QVariantMap>
#include <QTimer>
class WindowsNotifications:public QObject {
    Q_OBJECT
public:
    using Callback=void(*)(quintptr,const unsigned char*,size_t);
    using Show=void*(*)(const unsigned char*,size_t,Callback,quintptr);
    struct Transport{Show show=nullptr;void(*release)(void*)=nullptr;bool(*registerIdentity)()=nullptr;};
    explicit WindowsNotifications(QObject*parent=nullptr);
    WindowsNotifications(QObject*parent,Transport transport);
    ~WindowsNotifications();
    void notify(const QVariantMap&message);
    void activate(quint64 token,const QString&action);
    void clear();
signals:
    void activated(QString account,QString thread);void copyCode(QString code);void status(QString message);
private:
    struct Entry{QVariantMap message;void*handle=nullptr;qint64 expires=0;};
    QHash<quint64,Entry>entries;quint64 next=0;QLibrary library;QTimer expiry;bool identity=false;
    Show show=nullptr;void(*free)(void*)=nullptr;
    bool(*registerIdentity)()=nullptr;
    void retire(quint64 token);void expire();
};
