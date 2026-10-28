import QtQuick
Transition {
    enabled:Theme.motion
    SpringAnimation{property:"scale";to:.96;spring:7;damping:.8;epsilon:.002}
    SpringAnimation{property:"opacity";to:0;spring:8;damping:.8;epsilon:.005}
}
