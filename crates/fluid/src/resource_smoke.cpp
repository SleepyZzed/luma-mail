// Isolated lifecycle fixture for external process-tree counters. Single-shot
// phase boundaries do not introduce a sampling/polling timer into measured idle.
#include "bridge.h"
#include "html_surface.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QSaveFile>
#include <QJsonDocument>
#include <QFileInfo>
#include <cstdio>
class ResourceSmoke:public QObject {
    Bridge*mail;QQuickWindow*window;QTimer htmlReady;int stage=0;bool failed=false;QJsonArray checks;
    QQuickItem*find(QQuickItem*at,const QString&name){if(at->objectName()==name)return at;for(auto child:at->childItems())if(auto item=find(child,name))return item;return nullptr;}
    int boundRows(){auto list=find(window->contentItem(),"mailList");if(!list)return -1;auto content=list->property("contentItem").value<QQuickItem*>();int count=0;if(content)for(auto row:content->childItems())if(row->objectName().startsWith("mailRow-"))++count;return count;}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void announce(const char*name){std::printf("PERF bound_rows %d\nPERF %s\n",boundRows(),name);std::fflush(stdout);if(const auto path=qEnvironmentVariable("LUMA_RESOURCE_REPORT");!path.isEmpty()){QSaveFile file(QFileInfo(path).absolutePath()+"/phase.json");if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(QJsonObject{{"phase",QString::fromLatin1(name)},{"boundRows",boundRows()},{"loadedRows",mail->inbox()->rowCount()},{"nativeWindowsSurfaces",HtmlSurface::liveNativeSurfaces()}}).toJson());file.commit();}}}
    template<class F>void phase(const char*name,F next){announce(name);QTimer::singleShot(20000,this,[this,next]{announce("transition");next();});}
    void finish(){QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"note","Lifecycle assertions only. RAM/CPU are sampled externally for the entire process tree."}};if(auto path=qEnvironmentVariable("LUMA_RESOURCE_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}}mail->quit(failed?2:0);}
    void scroll(int index){if(index<1000){auto inbox=find(window->contentItem(),"inboxPane");QMetaObject::invokeMethod(inbox,"scrollToIndex",Q_ARG(QVariant,index));QTimer::singleShot(120,this,[this,index]{scroll(index+100);});return;}QTimer::singleShot(1500,this,[this]{check("scrolling 1000 loaded records retains fewer than 80 row delegates",boundRows()>0&&boundRows()<80);phase("scrolled_1000",[this]{auto inbox=find(window->contentItem(),"inboxPane");QMetaObject::invokeMethod(inbox,"revealIndex",Q_ARG(QVariant,0));htmlReady.start();});});}
public:
    ResourceSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        QTimer::singleShot(205000,this,[this]{check("resource lifecycle completes within deadline",false);finish();});
        connect(mail,&Bridge::ready,this,[this]{mail->send("settingsUpdate",{{"cache_mib",0}});QTimer::singleShot(3500,this,[this]{check("resource fixture uses an exposed native window",window->isExposed());phase("baseline",[this]{stage=1;mail->send("previewRows",{{"count",1000}});});});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){if(kind=="error"){check("resource fixture commands succeed: "+data.value("message").toString(),false);finish();}else if(kind=="rows"&&stage==1){stage=2;QTimer::singleShot(1500,this,[this]{check("large inbox keeps its metadata cap and virtualized rows",mail->inbox()->rowCount()==1000&&boundRows()>0&&boundRows()<80);phase("inbox_1000",[this]{scroll(100);});});}});
        htmlReady.setInterval(100);connect(&htmlReady,&QTimer::timeout,this,[this]{auto surface=find(window->contentItem(),"mailHtml-preview0");if(!surface||surface->implicitHeight()<600)return;htmlReady.stop();QTimer::singleShot(3500,this,[this]{check("newsletter uses the real private renderer",bool(find(window->contentItem(),"mailHtml-preview0")));phase("newsletter",[this]{mail->closeReader();QTimer::singleShot(3500,this,[this]{check("closing releases all native message bodies",mail->messages()->rowCount()==0&&!find(window->contentItem(),"mailHtml-preview0")&&HtmlSurface::liveNativeSurfaces()==0);phase("released",[this]{window->hide();QTimer::singleShot(3500,this,[this]{check("hiding stops foreground animation and photo work",!mail->windowVisible());phase("reclaimed_background",[this]{window->showNormal();QTimer::singleShot(3500,this,[this]{check("restoration reconstructs an exposed native scene after background release",window->isExposed()&&window->isSceneGraphInitialized());phase("restored",[this]{finish();});});});});});});});});});
    }
};
void startResourceSmoke(Bridge*m,QQuickWindow*w){new ResourceSmoke(m,w);}
