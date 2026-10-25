/* 本项目 ABI 探针；不修改上游源码。 */
#include <stddef.h>
#include "cs104_slave.h"
_Static_assert(sizeof(struct sCS101_AppLayerParameters) == 7 * sizeof(int), "application parameter ABI");
_Static_assert(sizeof(struct sCS104_APCIParameters) == 6 * sizeof(int), "APCI parameter ABI");
_Static_assert(sizeof(Lib60870VersionInfo) == 3 * sizeof(int), "version ABI");
_Static_assert(_Alignof(struct sCS101_AppLayerParameters) == _Alignof(int), "application alignment ABI");
_Static_assert(_Alignof(Lib60870VersionInfo) == _Alignof(int), "version alignment ABI");
_Static_assert(sizeof(bool) == 1, "C boolean ABI");
_Static_assert(sizeof(CS101_CauseOfTransmission) == sizeof(int), "COT enum ABI");
_Static_assert(sizeof(IEC60870_5_TypeID) == sizeof(int), "type enum ABI");
_Static_assert(sizeof(CS104_PeerConnectionEvent) == sizeof(int), "connection event ABI");
_Static_assert(offsetof(struct sCS101_AppLayerParameters, maxSizeOfASDU) == 6 * sizeof(int), "field offsets");
int essim_60870_header_version(void) {
    return LIB60870_VERSION_MAJOR * 10000 + LIB60870_VERSION_MINOR * 100 + LIB60870_VERSION_PATCH;
}
