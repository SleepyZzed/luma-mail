#include "bridge.h"
#include "rich_document.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QSaveFile>
#include <QFileInfo>
class PopoutSmoke:public QObject {
    Q_OBJECT
    Bridge*mail;QQuickWindow*main;QQuickItem*card=nullptr;RichDocument*document=nullptr;QString session,markup;QJsonArray checks;int stage=0;bool failed=false,finished=false;
    QQuickWindow*secondary(){for(auto w:QGuiApplication::allWindows())if(w->objectName()=="composerWindow")return qobject_cast<QQuickWindow*>(w);return nullptr;}
    void invoke(const char*name){QMetaObject::invokeMethod(card,name);}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void finish(){if(finished)return;finished=true;QJsonObject result{{"passed",!failed},{"checks",checks},{"syntheticData",true}};if(auto path=qEnvironmentVariable("LUMA_POPOUT_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(result).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(result).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
    void minimizeAndClose(QQuickWindow*host){
#ifdef Q_OS_WIN
        check("Windows pop out retains its native title bar",!host->flags().testFlag(Qt::FramelessWindowHint));
#else
        check("Linux pop out remains frameless for the compositor",host->flags().testFlag(Qt::FramelessWindowHint));
#endif
        host->showMinimized();QTimer::singleShot(250,this,[this,host]{invoke("focusEditor");QTimer::singleShot(400,this,[this,host]{check("repeated compose restores a minimized pop out",host->isVisible()&&host->visibility()==QWindow::Windowed&&document->html()==markup);main->hide();QTimer::singleShot(200,this,[this,host]{check("a visible pop out keeps foreground behavior while the inbox is hidden",mail->windowVisible()&&host->isVisible()&&!main->isVisible());stage=2;host->close();});});});
    }
    void detach(){invoke("toggleWindow");QTimer::singleShot(700,this,[this]{auto host=secondary();check("pop out moves the existing editor into a real native window",host&&host->isVisible()&&card->window()==host&&card->property("detached").toBool());check("pop out preserves the same rich document and unsaved session",mail->composer()["session"]==session&&document->html()==markup&&mail->composer()["draftId"].isNull());if(!host){finish();return;}host->resize(900,720);QTimer::singleShot(700,this,[this,host]{check("native resizing relays out the editor without glyph scaling",qAbs(card->width()-host->width())<2&&qAbs(card->height()-host->height())<2&&document->html()==markup);minimizeAndClose(host);});});}
public:
    PopoutSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),main(w){
        QTimer::singleShot(18000,this,[this]{if(!finished){check("pop out lifecycle finishes before its deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(300,this,[this]{mail->requestCompose();});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){
            if(finished)return;if(kind=="error"){check("pop out commands complete without an engine error: "+data["message"].toString(),false);finish();return;}
            if(kind=="composeOpened"&&stage==0){stage=1;session=data["session"].toString();card=main->findChild<QQuickItem*>("composer");document=main->findChild<RichDocument*>("composeDocument");if(!card||!document){check("native composer exists",false);finish();return;}QTimer::singleShot(700,this,[this]{main->findChild<QQuickItem*>("composeTo")->setProperty("text","person@example.com");QTextCursor cursor(document->nativeDocument());cursor.insertText("Pop out keeps this rich message");document->selection(0,7,7);document->toggle("bold");markup=document->html();invoke("sync");detach();});}
            else if(kind=="composeCloseQuestion"&&stage==2){stage=3;check("native close asks instead of silently drafting or discarding",secondary()&&card->property("shown").toBool()&&data["session"]==session&&mail->composer()["draftId"].isNull());QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("keep")));QTimer::singleShot(200,this,[this]{check("Keep editing preserves the detached document",secondary()&&card->window()==secondary()&&document->html()==markup);QMetaObject::invokeMethod(card,"returnHome",Q_ARG(QVariant,true));QTimer::singleShot(900,this,[this]{check("dock returns the same editor to the inbox window",card->window()==main&&!card->property("detached").toBool()&&!secondary()&&main->isVisible());check("dock retains rich content and engine ownership",document->html()==markup&&mail->composer()["session"]==session);invoke("toggleWindow");QTimer::singleShot(700,this,[this]{check("a composer can pop out again without duplicate windows",secondary()&&card->window()==secondary());stage=4;mail->send("settingsUpdate",{{"draft_mode","manual"}});});});});}
            else if(kind=="settings"&&stage==4){stage=5;auto host=secondary();if(host)host->close();else{check("second native window exists",false);finish();}}
            else if(kind=="composeClosed"&&stage==5){stage=6;QTimer::singleShot(1200,this,[this]{check("manual close destroys the pop out after its slide finishes",!secondary()&&!card->property("shown").toBool()&&card->window()==main);check("closed pop out releases document and view model content",document->nativeDocument()->isEmpty()&&mail->composer().isEmpty());const auto root=qEnvironmentVariable("LUMA_TEST_ROOT");check("pop out and close introduce no local mail or draft store",!QFileInfo::exists(root+"/cache/bodies")&&!QFileInfo::exists(root+"/config/composer"));finish();});}
        });
    }
};
void startPopoutSmoke(Bridge*m,QQuickWindow*w){new PopoutSmoke(m,w);}
#include "popout_smoke.moc"
