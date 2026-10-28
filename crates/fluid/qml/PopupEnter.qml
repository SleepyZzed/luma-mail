import QtQuick
Transition {
    enabled:Theme.motion
    SpringAnimation{property:"scale";from:.92;to:1;spring:5;damping:.55;epsilon:.002}
    SpringAnimation{property:"opacity";from:0;to:1;spring:6;damping:.7;epsilon:.005}
}
