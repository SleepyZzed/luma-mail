#include "bridge.h"
#include "html_surface.h"
#include <QQuickWindow>
#include <QQuickItem>
#include <QJsonDocument>
#include <QSaveFile>
#include <QFileInfo>
class HtmlSmoke:public QObject {
    Bridge*mail;QQuickWindow*window;QJsonArray checks;QTimer deadline,poll;bool failed=false,finished=false;int stage=0;
    QQuickItem*find(QQuickItem*at,const QString&name){if(at->objectName()==name)return at;for(auto child:at->childItems())if(auto item=find(child,name))return item;return nullptr;}
    void check(const QString&name,bool ok){checks.append(QJsonObject{{"check",name},{"passed",ok}});failed|=!ok;}
    void finish(){if(finished)return;finished=true;deadline.stop();poll.stop();QJsonObject report{{"passed",!failed},{"syntheticData",true},{"checks",checks}};if(failed){auto s=find(window->contentItem(),"mailHtml-preview0");auto row=mail->messages()->at(0);report["diagnostics"]=QJsonObject{{"surface",bool(s)},{"height",s?s->implicitHeight():0},{"viewport",s?s->property("viewportHeight").toReal():0},{"surfaceError",s?s->property("error").toString():""},{"hasHtml",row["hasHtml"].toBool()},{"documentBytes",row["document"].toString().size()},{"pending",row["documentPending"].toBool()}};}if(auto path=qEnvironmentVariable("LUMA_HTML_REPORT");!path.isEmpty()){QSaveFile f(path);if(f.open(QIODevice::WriteOnly)){f.write(QJsonDocument(report).toJson());f.commit();}}qInfo().noquote()<<QJsonDocument(report).toJson(QJsonDocument::Compact);mail->quit(failed?2:0);}
public:
    HtmlSmoke(Bridge*m,QQuickWindow*w):QObject(w),mail(m),window(w){
        deadline.setSingleShot(true);deadline.setInterval(20000);connect(&deadline,&QTimer::timeout,this,[this]{check("HTML reader completes within deadline",false);finish();});deadline.start();
        connect(mail,&Bridge::ready,this,[this]{QTimer::singleShot(350,this,[this]{auto list=window->findChild<QQuickItem*>("mailList");auto p=list->mapToScene(QPointF());mail->selectRow(0,0,p.x(),p.y(),list->width(),68);});});
        connect(mail,&Bridge::received,this,[this](const QString&kind,const QVariantMap&data){if(kind=="error"){check("reader command succeeds: "+data.value("message").toString(),false);finish();}});
        poll.setInterval(100);connect(&poll,&QTimer::timeout,this,[this]{
            auto surface=find(window->contentItem(),"mailHtml-preview0");
            if(stage==0&&surface&&!surface->property("error").toString().isEmpty()){check("native HTML surface opens: "+surface->property("error").toString(),false);finish();return;}
            if(stage==0&&surface&&surface->implicitHeight()>600){
                stage=1;check("newsletter uses full measured content height",true);
                check("renderer bounds stay limited to the visible reader",surface->property("viewportHeight").toReal()>0&&surface->property("viewportHeight").toReal()<surface->implicitHeight());
                auto data=mail->messages()->at(0);const auto html=data["document"].toString();
                check("sender active content and unapproved image URLs are removed",!html.contains("<script")&&!html.contains("<iframe")&&!html.contains("src=\"https://"));
                check("the themed document preserves table and safe blue links",html.contains("<table")&&html.contains("https://example.com/review")&&html.contains("color-scheme:"));
                auto settings=window->findChild<QQuickItem*>("settingsPanel");settings->setProperty("opened",true);
                QTimer::singleShot(200,this,[this]{auto s=find(window->contentItem(),"mailHtml-preview0");check("Settings suppresses the native HTML child overlay",s&&s->property("paused").toBool()&&
#ifdef Q_OS_WIN
                    !s->property("nativeVisible").toBool()
#else
                    true
#endif
                );window->findChild<QQuickItem*>("settingsPanel")->setProperty("opened",false);stage=2;});
            }else if(stage==2&&surface&&!surface->property("paused").toBool()){
                stage=3;window->setWidth(window->width()-140);
                QTimer::singleShot(900,this,[this]{auto s=find(window->contentItem(),"mailHtml-preview0");check("width changes remeasure and retain the reader",s&&s->implicitHeight()>600);mail->closeReader();stage=4;});
            }else if(stage==4&&!surface){check("closing the reader releases its renderer",true);check("HTML viewing creates no mailbox or draft cache",!QFileInfo::exists(qEnvironmentVariable("LUMA_TEST_ROOT")+"/cache/bodies")&&!QFileInfo::exists(qEnvironmentVariable("LUMA_TEST_ROOT")+"/config/composer"));finish();}
        });poll.start();
    }
};
void startHtmlSmoke(Bridge*m,QQuickWindow*w){new HtmlSmoke(m,w);}
