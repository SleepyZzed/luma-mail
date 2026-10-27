#include "bridge.h"
#include <QRegularExpression>
QVariantMap Bridge::actionState(const QVariantMap&requested)const{
    QVariantList rows;
    if(!m_selection.isEmpty()){for(int i=0;i<m_inbox.rowCount();++i){auto row=m_inbox.at(i);if(isSelected(row))rows.append(row);}}
    else{
        auto row=requested;const auto key=row.isEmpty()?m_reader.value("key").toMap():row.value("key").toMap();
        const int index=m_inbox.indexFor(key.value("account").toString()+QChar(0)+key.value("id").toString());
        if(index>=0)row=m_inbox.at(index);else if(row.isEmpty()&&key==m_openedRow.value("key").toMap())row=m_openedRow;
        if(!row.isEmpty())rows.append(row);
    }
    QVariantList keys;bool unread=false,starred=!rows.isEmpty(),important=!rows.isEmpty();
    for(const auto&value:rows){auto row=value.toMap();keys.append(row["key"]);unread|=row.value("unread").toBool();starred&=row.value("starred").toBool();important&=row.value("important").toBool();}
    const auto first=rows.isEmpty()?QVariantMap():rows.first().toMap();const auto key=first.value("key").toMap();
    const auto sender=first.value("sender").toString();const auto match=QRegularExpression("<([^<>]+)>").match(sender);const auto address=(match.hasMatch()?match.captured(1):sender).trimmed().toLower();bool connected=m_preview,blocked=false;
    for(const auto&value:m_settings.value("accounts").toList()){const auto account=value.toMap();if(account["id"]!=key["account"])continue;connected|=account.value("sender_controls").toBool();for(const auto&entry:account.value("blocked_senders").toList())blocked|=entry.toMap().value("address").toString()==address;}
    return {{"count",keys.size()},{"keys",keys},{"key",key},{"account",key.value("account")},{"sender",sender},{"address",address},{"ruleConnected",connected},{"blocked",blocked},{"unread",unread},{"starred",starred},{"important",important}};
}
void Bridge::composeFor(const QString&mode,const QVariantMap&key){if(m_selection.size()>1||key.isEmpty())return;send("composeStart",{{"mode",mode},{"key",key},{"account",key.value("account")}});}
