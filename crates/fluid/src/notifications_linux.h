#pragma once
#include <QObject>
#include <QVariantMap>
#include <QHash>
#include <QQueue>
class LinuxNotifications:public QObject {
    Q_OBJECT
public:
    explicit LinuxNotifications(QObject*parent=nullptr);
    void notify(const QVariantMap&message);
signals:
    void activated(QString account,QString thread);void status(QString message);
private slots:
    void invoked(uint id,const QString&action);void closed(uint id,uint reason);
private:
    QHash<uint,QVariantMap>targets;QQueue<uint>order;int pending=0;
};
