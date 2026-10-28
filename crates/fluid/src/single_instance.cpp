#include "single_instance.h"
#include <QLocalSocket>
#include <QCryptographicHash>
#include <QStandardPaths>
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTimer>
#include <QDeadlineTimer>
namespace {constexpr qsizetype Limit=65536+256;}
SingleInstance::SingleInstance(const QString&scope,QObject*parent):QObject(parent){
    const auto hash=QString::fromLatin1(QCryptographicHash::hash(scope.toUtf8(),QCryptographicHash::Sha256).toHex().left(32));
    auto runtime=QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);if(runtime.isEmpty())runtime=QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation);
    QDir().mkpath(runtime);
#ifdef Q_OS_UNIX
    name=QDir(runtime).filePath("luma-mail-"+hash);
#else
    name="luma-mail-"+hash;
#endif
    lock=std::make_unique<QLockFile>(QDir(runtime).filePath("luma-mail-"+hash+".lock"));lock->setStaleLockTime(0);
    server.setSocketOptions(QLocalServer::UserAccessOption);server.setMaxPendingConnections(4);connect(&server,&QLocalServer::newConnection,this,&SingleInstance::accept);
}
SingleInstance::~SingleInstance(){if(primary){server.close();QLocalServer::removeServer(name);}}
bool SingleInstance::forward(const QString&uri){
    QLocalSocket socket;socket.setReadBufferSize(64);socket.connectToServer(name);if(!socket.waitForConnected(300))return false;
    const auto frame=QJsonDocument(QJsonObject{{"version",1},{"uri",uri}}).toJson(QJsonDocument::Compact)+'\n';
    if(frame.size()>Limit)return false;
    if(socket.write(frame)!=frame.size())return false;
    if(socket.bytesToWrite()>0&&!socket.waitForBytesWritten(500))return false;
    QDeadlineTimer deadline(1000);QByteArray answer;
    while(!answer.contains('\n')&&!deadline.hasExpired()){
        if(socket.bytesAvailable()==0)socket.waitForReadyRead(int(deadline.remainingTime()));
        // The peer can close directly after writing its acknowledgement. Read
        // buffered bytes even when waitForReadyRead reports that disconnection.
        answer.append(socket.readAll());if(answer.size()>16)return false;
        if(socket.state()==QLocalSocket::UnconnectedState)break;
    }
    return answer=="ok\n";
}
SingleInstance::Result SingleInstance::start(const QString&uri){
    if(uri.toUtf8().size()>65536)return Busy;
    if(!lock->tryLock(0))return forward(uri)?Forwarded:Busy;
    // Only the owner of this user's process lock may remove a stale endpoint.
    QLocalServer::removeServer(name);
    if(!server.listen(name)){lock->unlock();return Busy;}
    primary=true;return Primary;
}
void SingleInstance::accept(){
    while(auto socket=server.nextPendingConnection()){
        if(active>=4){socket->abort();socket->deleteLater();continue;}
        ++active;connect(socket,&QObject::destroyed,this,[this]{--active;});
        socket->setParent(this);socket->setReadBufferSize(Limit+1);auto bytes=std::make_shared<QByteArray>();
        auto deadline=new QTimer(socket);deadline->setSingleShot(true);deadline->setInterval(1500);connect(deadline,&QTimer::timeout,socket,[socket]{socket->abort();socket->deleteLater();});deadline->start();
        connect(socket,&QLocalSocket::disconnected,socket,&QObject::deleteLater);
        auto handled=std::make_shared<bool>(false);
        auto receive=[this,socket,bytes,deadline,handled]{
            if(*handled)return;
            bytes->append(socket->readAll());if(bytes->size()>Limit){socket->abort();return;}
            if(!bytes->contains('\n'))return;
            if(bytes->indexOf('\n')!=bytes->size()-1){socket->abort();return;}
            QJsonParseError error;const auto document=QJsonDocument::fromJson(*bytes,&error);const auto data=document.object();const auto uri=data["uri"].toString();
            if(error.error!=QJsonParseError::NoError||!document.isObject()||data.size()!=2||data["version"].toInt()!=1||!data["uri"].isString()||uri.toUtf8().size()>65536||(!uri.isEmpty()&&!uri.startsWith("mailto:",Qt::CaseInsensitive))){socket->abort();return;}
            *handled=true;emit activated(uri);socket->write("ok\n");socket->flush();
            // Let the client close after reading the acknowledgement. Closing
            // the server's Windows named pipe here can discard its final bytes
            // before the client's waitForReadyRead observes them. The existing
            // deadline still bounds peers that never consume/close the socket.
            deadline->start();
        };
        connect(socket,&QLocalSocket::readyRead,socket,receive);
        if(socket->bytesAvailable()>0)QTimer::singleShot(0,socket,receive);
    }
}
