#include "notifications_linux.h"
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QTimer>
#include <QDateTime>
#include <algorithm>
namespace {const QString Service="org.freedesktop.Notifications",Path="/org/freedesktop/Notifications",Interface="org.freedesktop.Notifications";}
LinuxNotifications::LinuxNotifications(QObject*parent):QObject(parent){
    expiry.setSingleShot(true);connect(&expiry,&QTimer::timeout,this,&LinuxNotifications::expire);
    auto bus=QDBusConnection::sessionBus();bus.connect(Service,Path,Interface,"ActionInvoked",this,SLOT(invoked(uint,QString)));bus.connect(Service,Path,Interface,"NotificationClosed",this,SLOT(closed(uint,uint)));
}
void LinuxNotifications::notify(const QVariantMap&message){
    if(pending>=64){emit status("The notification service is busy. New mail remains in your inbox.");return;}
    auto call=QDBusMessage::createMethodCall(Service,Path,Interface,"Notify");
    const QVariantMap hints{{"desktop-entry","io.github.lumamail.Luma"},{"category","email.arrived"},{"urgency",QVariant::fromValue(uchar(1))}};
    QStringList actions{"default","Open email","open","Open"};
    const auto code=message.value("code").toString();if(code.size()>=4&&code.size()<=8&&std::all_of(code.begin(),code.end(),[](QChar c){return c>='0'&&c<='9';}))actions<<"copy-code"<<"Copy code";
    call.setArguments({"Luma Mail",QVariant::fromValue(uint(0)),"io.github.lumamail.Luma",message.value("title").toString(),message.value("body").toString().toHtmlEscaped(),actions,hints,-1});
    ++pending;auto watch=new QDBusPendingCallWatcher(QDBusConnection::sessionBus().asyncCall(call,5000),this);
    connect(watch,&QDBusPendingCallWatcher::finished,this,[this,watch,message]{--pending;QDBusPendingReply<uint>reply=*watch;watch->deleteLater();
        if(reply.isError()){emit status("System notification daemon unavailable: "+reply.error().message());return;}
        const auto id=reply.value();if(id==0){emit status("The system notification daemon returned an invalid notification ID.");return;}
        if(!targets.contains(id))order.enqueue(id);targets[id]={{"account",message.value("account")},{"thread",message.value("thread")},{"code",message.value("code")},{"expires",QDateTime::currentMSecsSinceEpoch()+600000}};while(order.size()>64)targets.remove(order.dequeue());
        expire();emit status(QString());
    });
}
void LinuxNotifications::invoked(uint id,const QString&action){if(!targets.contains(id))return;const auto target=targets[id];if(action=="default"||action=="open")emit activated(target["account"].toString(),target["thread"].toString());else if(action=="copy-code"&&target["expires"].toLongLong()>QDateTime::currentMSecsSinceEpoch()){const auto code=target["code"].toString();if(code.size()>=4&&code.size()<=8&&std::all_of(code.begin(),code.end(),[](QChar c){return c>='0'&&c<='9';})){targets[id].remove("code");emit copyCode(code);}}}
void LinuxNotifications::closed(uint id,uint){targets.remove(id);order.removeAll(id);expire();}
void LinuxNotifications::expire(){const auto now=QDateTime::currentMSecsSinceEpoch();qint64 next=0;for(auto i=targets.begin();i!=targets.end();++i){if(i->value("code").toString().isEmpty())continue;const auto deadline=i->value("expires").toLongLong();if(deadline<=now)i->remove("code");else if(!next||deadline<next)next=deadline;}if(next)expiry.start(int(qMax<qint64>(1,next-now)));else expiry.stop();}
