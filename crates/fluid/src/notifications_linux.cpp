#include "notifications_linux.h"
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
namespace {const QString Service="org.freedesktop.Notifications",Path="/org/freedesktop/Notifications",Interface="org.freedesktop.Notifications";}
LinuxNotifications::LinuxNotifications(QObject*parent):QObject(parent){
    auto bus=QDBusConnection::sessionBus();bus.connect(Service,Path,Interface,"ActionInvoked",this,SLOT(invoked(uint,QString)));bus.connect(Service,Path,Interface,"NotificationClosed",this,SLOT(closed(uint,uint)));
}
void LinuxNotifications::notify(const QVariantMap&message){
    if(pending>=64){emit status("The notification service is busy. New mail remains in your inbox.");return;}
    auto call=QDBusMessage::createMethodCall(Service,Path,Interface,"Notify");
    const QVariantMap hints{{"desktop-entry","io.github.lumamail.Luma"},{"category","email.arrived"},{"urgency",QVariant::fromValue(uchar(1))}};
    call.setArguments({"Luma Mail",QVariant::fromValue(uint(0)),"io.github.lumamail.Luma",message.value("title").toString(),message.value("body").toString().toHtmlEscaped(),QStringList{"default","Open email"},hints,-1});
    ++pending;auto watch=new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(call,5000),this);
    connect(watch,&QDBusPendingCallWatcher::finished,this,[this,watch,message]{--pending;QDBusPendingReply<uint>reply=*watch;watch->deleteLater();
        if(reply.isError()){emit status("System notification daemon unavailable: "+reply.error().message());return;}
        const auto id=reply.value();if(id==0){emit status("The system notification daemon returned an invalid notification ID.");return;}
        if(!targets.contains(id))order.enqueue(id);targets[id]={{"account",message.value("account")},{"thread",message.value("thread")}};while(order.size()>64)targets.remove(order.dequeue());emit status(QString());
    });
}
void LinuxNotifications::invoked(uint id,const QString&action){if(action!="default"||!targets.contains(id))return;const auto target=targets[id];emit activated(target["account"].toString(),target["thread"].toString());}
void LinuxNotifications::closed(uint id,uint){targets.remove(id);order.removeAll(id);}
