#pragma once
#include <QObject>
#include <QLocalServer>
#include <QLockFile>
#include <memory>
class SingleInstance:public QObject {
    Q_OBJECT
public:
    enum Result {Primary,Forwarded,Busy};
    explicit SingleInstance(const QString&scope,QObject*parent=nullptr);
    ~SingleInstance()override;
    Result start(const QString&uri=QString());
    QString endpoint()const{return name;}
signals:void activated(QString uri);
private:
    bool forward(const QString&uri);void accept();
    QString name;QLocalServer server;std::unique_ptr<QLockFile>lock;bool primary=false;int active=0;
};
