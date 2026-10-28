#include "bridge.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QJsonDocument>
#include <QSaveFile>
#include <QFileInfo>
#include <QDesktopServices>
#include <QMouseEvent>
#include <functional>
class SyncSmoke:public QObject {
    Q_OBJECT
    Bridge*mail;QQuickWindow*window;QJsonArray checks;bool failed=false,finished=false,started=false;
    QStringList openedLinks;
    QQuickItem*find(QQuickItem*at,const QString&name){if(at->objectName()==name)return at;for(auto child:at->childItems())if(auto item=find(child,name))return item;return nullptr;}
    void check(const QString&name,bool passed){checks.append(QJsonObject{{"check",name},{"passed",passed}});failed|=!passed;}
    void finish(){if(finished)return;finished=true;QDesktopServices::unsetUrlHandler("https");QJsonObject report{{"passed",!failed},{"checks",checks},{"syntheticData",true},{"note","Native incremental-event wiring through the real preview pipe. Provider history requests and notification transport have separate tests."}};if(auto path=qEnvironmentVariable("LUMA_SYNC_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(report).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
    void clickReceivedLink(){
        auto text=find(window->contentItem(),"readBody-preview1");QRectF rectangle;
        const auto source=mail->messages()->at(0)["text"].toString();const auto offset=source.indexOf("https://")+2;
        bool located=text&&QMetaObject::invokeMethod(text,"positionToRectangle",Q_RETURN_ARG(QRectF,rectangle),Q_ARG(int,offset));check("received URL has a native hit target",located);if(!located){finish();return;}
        auto point=text->mapToScene(rectangle.center());
        QMouseEvent down(QEvent::MouseButtonPress,point,QPointF(window->mapToGlobal(point.toPoint())),Qt::LeftButton,Qt::LeftButton,Qt::ControlModifier);QCoreApplication::sendEvent(window,&down);QMouseEvent up(QEvent::MouseButtonRelease,point,QPointF(window->mapToGlobal(point.toPoint())),Qt::LeftButton,Qt::NoButton,Qt::ControlModifier);QCoreApplication::sendEvent(window,&up);
        QTimer::singleShot(150,this,[this]{check("Ctrl click opens one safe received link",openedLinks==QStringList{"https://example.com/review"});mail->clearSelection();auto menu=window->findChild<QObject*>("messageActionsMenu");check("individual-message actions have a dedicated menu",menu);if(!menu){finish();return;}menu->setProperty("message",mail->messages()->at(0));menu->setProperty("threadKey",mail->reader()["key"]);QMetaObject::invokeMethod(menu,"perform",Q_ARG(QVariant,QString("unstar")));});
    }
    void awaitState(QWindow::Visibility target,const QString&name,std::function<void()> next,int attempts=40){
        if(window->visibility()==target){QTimer::singleShot(150,this,[this,target,name,next,attempts]{if(window->visibility()==target){check(name,true);next();}else awaitState(target,name,next,attempts-1);});return;}
        if(!attempts){check(name+QString(" (visibility %1, native state %2)").arg(int(window->visibility())).arg(int(window->windowState())),false);finish();return;}
        QTimer::singleShot(50,this,[this,target,name,next,attempts]{awaitState(target,name,next,attempts-1);});
    }
    void restoreWindow(){
        window->showMinimized();
        awaitState(QWindow::Minimized,"native desktop accepts minimization",[this]{
            emit mail->showRequested();
            awaitState(QWindow::Windowed,"tray and notification activation restore a minimized window",[this]{
                window->showMaximized();
                awaitState(QWindow::Maximized,"native desktop accepts maximization",[this]{
                    window->hide();emit mail->showRequested();
                    awaitState(QWindow::Maximized,"tray activation preserves a maximized window",[this]{finish();});
                });
            });
        });
    }
    void inject(const QString&event,const QJsonObject&data){mail->receive({{"id",0},{"event",event},{"data",data}});}
    void exercise(){
        const auto body=mail->messages()->at(0)["text"].toString();const auto reader=mail->reader()["key"].toMap();const auto activity=mail->activityKind();const auto row=mail->inbox()->at(5);
        mail->selectRow(5,Qt::ControlModifier,0,0,0,0);auto incoming=QJsonObject::fromVariantMap(row);incoming["date"]=qint64(9000000000000LL);incoming["unread"]=true;
        inject("rowChanged",{{"row",incoming},{"insert",true},{"generation",qint64(mail->viewRequest)}});
        check("incoming reply moves one existing conversation",mail->inbox()->rowCount()==8&&mail->inbox()->at(0)["key"]==row["key"]);
        check("incoming metadata preserves open body and reader",mail->reader()["key"].toMap()==reader&&mail->messages()->at(0)["text"].toString()==body);
        check("selection follows the conversation after reordering",mail->selectionCount()==1&&mail->isSelected(mail->inbox()->at(0)));
        mail->selectRow(2,Qt::ShiftModifier,0,0,0,0);check("Shift range uses the moved anchor",mail->selectionCount()==3);
        const QJsonObject reply{{"id","preview-reply"},{"from","Maya Chen <maya@example.com>"},{"to","alex@example.com"},{"date",qint64(9000000000000LL)},{"unread",true}};
        inject("threadUpdated",{{"key",QJsonObject::fromVariantMap(reader)},{"reading",qint64(mail->readerRequest)},{"messages",QJsonArray{QJsonObject{{"id","preview1"},{"starred",true}},reply}}});
        check("reply headers keep the opened message content",mail->messages()->rowCount()==2&&mail->messages()->at(0)["text"].toString()==body);
        check("new reply starts unread without downloading its body",mail->messages()->at(1)["unread"].toBool()&&!mail->messages()->at(1).contains("text"));
        inject("threadUpdated",{{"key",QJsonObject::fromVariantMap(reader)},{"reading",qint64(mail->readerRequest-1)},{"messages",QJsonArray{QJsonObject{{"id","preview1"},{"starred",false}}}}});check("stale reader updates cannot change the current message headers",mail->messages()->at(0)["starred"].toBool());
        inject("sync",{{"silent",true},{"lastChecked",1234},{"unread",QJsonObject{{"preview",42}}}});
        check("silent checks leave the loading indicator and selection intact",mail->activityKind()==activity&&mail->selectionCount()==3&&mail->sync()["unread"].toMap()["preview"].toInt()==42);
        QTimer::singleShot(500,this,[this]{auto original=find(window->contentItem(),"threadMessage-preview1"),reply=find(window->contentItem(),"threadMessage-preview-reply");check("incoming reply leaves the current message expanded",original&&original->property("expanded").toBool());check("incoming reply waits for explicit opening",reply&&!reply->property("expanded").toBool());const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("incremental updates create no mail or draft store",!QFileInfo::exists(root+"/cache/bodies")&&!QFileInfo::exists(root+"/config/composer"));auto reader=find(window->contentItem(),"readerPane");QMetaObject::invokeMethod(reader,"showDetails",Q_ARG(QVariant,QString("preview1")));QTimer::singleShot(450,this,[this,reader]{auto details=find(window->contentItem(),"messageDetails-preview1");check("message details reveal literal sender and recipient headers",details&&details->property("text").toString().contains("To: alex@example.com"));QMetaObject::invokeMethod(reader,"showDetails",Q_ARG(QVariant,QString("preview1")));QTimer::singleShot(700,this,[this]{check("closed message details release the extra text document",!find(window->contentItem(),"messageDetails-preview1"));clickReceivedLink();});});});
    }
public:
    Q_SLOT void handle(const QUrl&url){openedLinks.append(url.toString());}
    SyncSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){QTimer::singleShot(12000,this,[this]{if(!finished){check("sync test completes within deadline",false);finish();}});
        QDesktopServices::setUrlHandler("https",this,"handle");
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(350,this,[this]{auto inbox=window->findChild<QQuickItem*>("inboxPane");QMetaObject::invokeMethod(inbox,"openIndex",Q_ARG(QVariant,1),Q_ARG(QVariant,0));});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){if(finished)return;if(kind=="error"){check("sync wiring commands succeed: "+data.value("message").toString(),false);finish();}else if(kind=="mutation"){check("single-message actions immediately update reader flags",!mail->messages()->at(0)["starred"].toBool()&&!mail->messages()->at(0)["text"].toString().isEmpty());restoreWindow();}else if(kind=="body"&&!started){started=true;QTimer::singleShot(1000,this,[this]{exercise();});}});
    }
};
void startSyncSmoke(Bridge*m,QQuickWindow*w){new SyncSmoke(m,w);}
#include "sync_smoke.moc"
