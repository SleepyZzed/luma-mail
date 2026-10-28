#include "runtime_cleanup.h"
#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QTimer>
#include <QElapsedTimer>
#include <QThread>
#include <QHash>
#include <algorithm>
namespace {
bool ownedPath(const QString&path){
    QFileInfo info(path);const auto name=info.fileName();
    return name.startsWith("luma-webview-")&&name.size()==19&&!info.isSymLink()&&!info.isJunction()&&
        info.absoluteDir().canonicalPath()==QDir(QDir::tempPath()).canonicalPath()&&info.canonicalFilePath()==info.absoluteFilePath();
}
bool removeRuntime(const QString&path){return !QFileInfo::exists(path)||(ownedPath(path)&&QDir(path).removeRecursively());}
class Cleanup:public QObject {
    QHash<QString,int> pending;QTimer timer;int delay=100;
    void sweep(){bool retry=false;for(auto i=pending.begin();i!=pending.end();){if(removeRuntime(i.key()))i=pending.erase(i);else{retry|=++i.value()<10;++i;}}if(retry){delay=std::min(delay*2,2000);timer.start(delay);}else delay=100;}
public:
    Cleanup():QObject(qApp){timer.setSingleShot(true);connect(&timer,&QTimer::timeout,this,[this]{sweep();});
        connect(qApp,&QCoreApplication::aboutToQuit,this,[this]{timer.stop();QElapsedTimer wait;wait.start();while(!pending.isEmpty()&&wait.elapsed()<2000){sweep();if(!pending.isEmpty())QThread::msleep(25);}timer.stop();});}
    void retire(const QString&path){if(!ownedPath(path))return;if(!pending.contains(path)){if(pending.size()>=256)pending.erase(pending.begin());pending.insert(path,0);}if(!timer.isActive())timer.start(100);}
};
Cleanup&cleanup(){static auto instance=new Cleanup;return *instance;}
}
ReaderRuntime::ReaderRuntime(){
    directory=std::make_unique<QTemporaryDir>(QDir::tempPath()+"/luma-webview-XXXXXX");
    if(directory->isValid()){directory->setAutoRemove(false);lock=std::make_unique<QLockFile>(directory->path()+"/owner.lock");if(!lock->tryLock(0)){directory->remove();directory.reset();}}
    cleanup();
}
ReaderRuntime::~ReaderRuntime(){if(directory&&directory->isValid()){lock.reset();cleanup().retire(directory->path());}}
bool ReaderRuntime::isValid()const{return directory&&directory->isValid()&&lock&&lock->isLocked();}
QString ReaderRuntime::path()const{return directory?directory->path():QString();}
void ReaderRuntime::recover(){
    for(const auto&info:QDir(QDir::tempPath()).entryInfoList({"luma-webview-??????"},QDir::Dirs|QDir::NoDotAndDotDot)){
        if(!ownedPath(info.absoluteFilePath()))continue;QLockFile owner(info.absoluteFilePath()+"/owner.lock");if(owner.tryLock(0)){owner.unlock();cleanup().retire(info.absoluteFilePath());}
    }
}
