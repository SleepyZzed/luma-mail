#include "bridge.h"
QVariantList Bridge::searchAccounts()const{
    if(searchSubset)return scopedAccounts;if(!m_account.isEmpty())return {m_account};
    QVariantList ids;for(const auto&value:m_settings.value("accounts").toList()){const auto account=value.toMap();if(account.value("enabled").toBool())ids.append(account.value("id"));}return ids;
}
void Bridge::startView(const QString&folder,const QString&account,const QString&query,const QVariant&scope){
    m_folder=folder;m_account=account;m_search=query;clearSelection();m_readerOpen=false;openedKey.clear();m_reader.clear();m_messages.replace({});emit readerChanged();emit viewChanged();QVariantMap data{{"folder",folder},{"search",query}};if(scope.isValid())data["accounts"]=scope;viewRequest=send("view",data);
}
void Bridge::setView(const QString&folder,const QString&account,const QString&query){searchSubset=false;scopedAccounts.clear();startView(folder,account,query,account.isEmpty()?QVariant():QVariant(QVariantList{account}));}
void Bridge::refreshView(){if(searchSubset)startView(m_folder,m_account,m_search,scopedAccounts);else setView(m_folder,m_account,m_search);}
void Bridge::searchCurrent(const QString&query){m_search=query;refreshView();}
void Bridge::applySearch(const QJsonObject&data){
    searchRequest=0;
    if(data["accounts"].isArray()){
        searchSubset=true;scopedAccounts=data["accounts"].toArray().toVariantList();const QString account=scopedAccounts.size()==1?scopedAccounts.first().toString():QString();
        startView(data["folder"].toString(),account,data["query"].toString(),scopedAccounts);
    }else setView(data["folder"].toString(),QString(),data["query"].toString());
}
void Bridge::cancelSearch(){if(searchRequest)send("cancel",{{"request",searchRequest}});searchRequest=0;}
