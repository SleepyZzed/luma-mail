#include "bridge.h"
#include "rich_document.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QJsonDocument>
#include <QSaveFile>
#include <QColor>

// Exposed-window regressions: the original menu tests called open() directly
// and never checked whether a pointer-opened menu had usable painted bounds.
class PopupSmoke final:public QObject {
    Q_OBJECT
    Bridge*mail;QQuickWindow*window;QQuickItem*card=nullptr;QObject*menu=nullptr;
    QJsonArray checks;bool failed=false,finished=false;int stage=0;
    QQuickItem*findIn(QQuickItem*item,const QString&name){if(item->objectName()==name)return item;for(auto child:item->childItems())if(auto found=findIn(child,name))return found;return nullptr;}
    QQuickItem*find(const QString&name){return findIn(window->contentItem(),name);}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void invoke(QObject*object,const char*name){check(QString("action exists: ")+name,object&&QMetaObject::invokeMethod(object,name));}
    void click(QQuickItem*item,Qt::MouseButton button){
        if(!item){check("pointer target exists",false);return;}
        const QPointF p=item->mapToScene(QPointF(qMin(item->width()/2,80.),qMin(item->height()/2,20.)));
        const QPointF global=window->mapToGlobal(p.toPoint());
        QMouseEvent down(QEvent::MouseButtonPress,p,global,button,button,Qt::NoModifier);QCoreApplication::sendEvent(window,&down);
        QMouseEvent up(QEvent::MouseButtonRelease,p,global,button,Qt::NoButton,Qt::NoModifier);QCoreApplication::sendEvent(window,&up);
    }
    void key(int code,Qt::KeyboardModifiers modifiers=Qt::NoModifier){QKeyEvent down(QEvent::KeyPress,code,modifiers);QCoreApplication::sendEvent(window,&down);QKeyEvent up(QEvent::KeyRelease,code,modifiers);QCoreApplication::sendEvent(window,&up);}
    bool usable(QObject*popup){return popup&&popup->property("visible").toBool()&&popup->property("width").toReal()>=220&&popup->property("height").toReal()>80&&popup->property("popupType").toInt()==0;}
    void screenshot(const QString&name){const auto prefix=qEnvironmentVariable("LUMA_POPUP_SCREENSHOT");if(!prefix.isEmpty())window->grabWindow().save(prefix+"-"+name+".png");}
    void finish(){if(finished)return;finished=true;QJsonObject result{{"passed",!failed},{"checks",checks},{"syntheticData",true}};if(auto path=qEnvironmentVariable("LUMA_POPUP_REPORT");!path.isEmpty()){QSaveFile file(path);if(file.open(QIODevice::WriteOnly)){file.write(QJsonDocument(result).toJson());file.commit();}}qInfo().noquote()<<QJsonDocument(result).toJson(QJsonDocument::Compact);mail->send("settingsUpdate",{{"draft_mode","manual"}});mail->quit(failed?2:0);}
    void compose(){stage=1;mail->requestCompose();}
public:
    PopupSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        QTimer::singleShot(24000,this,[this]{if(!finished){check("popup workflows finish before deadline",false);finish();}});
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(400,this,[this]{
            auto button=find("newMessageButton");auto content=button?button->findChild<QQuickItem*>("buttonContents"):nullptr;
            check("Compose has centered content with balanced horizontal breathing room",button&&content&&content->mapToItem(button,QPointF()).x()>=14&&qAbs(content->mapToItem(button,QPointF()).x()-(button->width()-content->width())/2)<1);
            menu=window->findChild<QObject*>("mailActionsMenu");
            const auto id=mail->inbox()->at(0)["key"].toMap()["id"].toString();click(find("mailRow-"+id),Qt::RightButton);
            QTimer::singleShot(550,this,[this]{
                check("right-clicking an inbox row opens a readable in-scene menu",usable(menu));
                check("right-click targets that row without opening its body",menu&&menu->property("state").toMap()["count"].toInt()==1&&!mail->readerOpen());screenshot("inbox");
                invoke(menu,"close");auto list=find("mailList");if(list)list->forceActiveFocus();key(Qt::Key_F10,Qt::ShiftModifier);
                QTimer::singleShot(450,this,[this]{check("Shift F10 opens the same inbox context actions",usable(menu));invoke(menu,"close");compose();});
            });
        });});
        connect(mail,&Bridge::received,this,[this](const QString&event,const QVariantMap&data){
            if(finished)return;
            if(event=="error"){check("no unexpected engine error: "+data["message"].toString(),false);finish();return;}
            if(event=="composeOpened"&&stage==1){stage=2;QTimer::singleShot(800,this,[this]{
                card=find("composer");auto body=find("composeBody");auto document=window->findChild<RichDocument*>("composeDocument");auto to=find("composeTo");
                if(!body||!document||!to){check("composer controls load",false);finish();return;}
                to->setProperty("text","person@example.com");QTextCursor cursor(document->nativeDocument());cursor.insertText("Keep this unsaved text");body->forceActiveFocus();QMetaObject::invokeMethod(body,"selectAll");click(body,Qt::RightButton);
                QTimer::singleShot(550,this,[this]{auto editorMenu=window->findChild<QObject*>("composeBodyContextMenu");check("editor right-click opens a themed menu with normal bounds",usable(editorMenu));screenshot("editor");key(Qt::Key_Escape);QTimer::singleShot(450,this,[this]{auto context=window->findChild<QObject*>("composeBodyContextMenu");check("Escape dismisses the edit menu without closing the composer",(!context||!context->property("visible").toBool())&&card->property("shown").toBool());invoke(card,"requestClose");});});
            });}
            else if(event=="composeCloseQuestion"&&stage==2){stage=3;QTimer::singleShot(550,this,[this]{
                auto question=card->property("closeDialog").value<QObject*>();auto background=question?question->property("background").value<QQuickItem*>():nullptr;
                check("save-draft question uses the rounded in-scene themed dialog",usable(question)&&background&&background->property("radius").toReal()>=18&&background->property("color").value<QColor>()!=QColor(Qt::black));
                check("asking to close has not created a Gmail draft",mail->composer()["draftId"].isNull());screenshot("draft");
                QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("keep")));
                QTimer::singleShot(500,this,[this]{
                    auto doc=window->findChild<RichDocument*>("composeDocument");check("Keep editing retains authored text without a draft",doc&&doc->nativeDocument()->toPlainText().contains("Keep this unsaved text")&&mail->composer()["draftId"].isNull());
                    invoke(find("composeTable"),"clicked");QTimer::singleShot(450,this,[this]{auto table=window->findChild<QObject*>("composeTableDialog");auto footer=table?table->property("footer").value<QQuickItem*>():nullptr;check("formatting dialogs retain a visible standard-button footer",usable(table)&&footer&&footer->isVisible()&&footer->height()>30);invoke(table,"reject");stage=4;mail->send("settingsUpdate",{{"draft_mode","manual"}});});
                });
            });}
            else if(event=="settings"&&stage==4){stage=5;invoke(card,"requestClose");}
            else if(event=="composeClosed"&&stage==5){stage=6;QTimer::singleShot(1100,this,[this]{check("all temporary editing menus release with the closed composer",!find("composeBody")&&!window->findChild<QObject*>("composeBodyContextMenu"));mail->send("settingsUpdate",{{"draft_mode","ask"}});mail->selectRow(0,0,240,160,450,68);});}
            else if(event=="body"&&stage==6){stage=7;mail->requestCompose("reply",mail->messages()->at(0));}
            else if(event=="composeOpened"&&stage==7){stage=8;QTimer::singleShot(700,this,[this]{
                auto document=window->findChild<RichDocument*>("composeDocument");if(!document){check("inline editor exists",false);finish();return;}
                check("reply starts inside the thread",card->property("inlineMode").toBool());QTextCursor cursor(document->nativeDocument());cursor.insertText("An unsaved inline reply");
                mail->selectRow(1,0,240,233,450,68);
            });}
            else if(event=="composeCloseQuestion"&&stage==8){stage=9;QTimer::singleShot(450,this,[this]{
                auto document=window->findChild<RichDocument*>("composeDocument");check("changing threads uses Ask policy without silently drafting or losing reply text",document&&document->nativeDocument()->toPlainText().contains("An unsaved inline reply")&&!card->property("inlineMode").toBool()&&mail->composer()["draftId"].isNull());
                QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("keep")));QTimer::singleShot(500,this,[this]{check("Keep editing preserves the outgoing reply after navigation",card->property("shown").toBool()&&!mail->composer().isEmpty());invoke(card,"requestClose");});
            });}
            else if(event=="composeCloseQuestion"&&stage==9){stage=10;QMetaObject::invokeMethod(card,"answerClose",Q_ARG(QVariant,QString("close")));}
            else if(event=="composeClosed"&&stage==10){stage=11;QTimer::singleShot(1100,this,[this]{check("explicitly closing the outgoing reply releases its controls",!find("composeBody")&&mail->composer().isEmpty());finish();});}
        });
    }
};
void startPopupSmoke(Bridge*mail,QQuickWindow*window){new PopupSmoke(mail,window);}
#include "popup_smoke.moc"
