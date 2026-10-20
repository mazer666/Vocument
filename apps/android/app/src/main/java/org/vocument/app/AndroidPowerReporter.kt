package org.vocument.app

import android.content.Context
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import uniffi.vocument_ffi.FfiPowerLimits
import uniffi.vocument_ffi.FfiPowerState

/**
 * Reports the energy and thermal state of the device to the shared core
 * (`POWER-001`, `BIGMOBILE-001`).
 *
 * The shell only **reports** the state; the core decides what it means. That
 * split keeps the decision testable and guarantees that macOS and Android
 * behave the same way.
 */
object AndroidPowerReporter {

    /**
     * Reads the current state and reports it to the core.
     *
     * Returns the limits that the state implies, so that the caller can show
     * why the background work is currently reduced.
     */
    fun report(context: Context): FfiPowerLimits? {
        val state = readState(context) ?: return null
        return runCatching { DocumentCoreBridge().reportPowerState(state) }.getOrNull()
    }

    /**
     * Reads the state of the device without reporting it.
     *
     * The thermal level follows the Android `THERMAL_STATUS_*` levels, so that
     * the value can be passed through without a translation.
     */
    fun readState(context: Context): FfiPowerState? {
        val batteryManager = context.getSystemService(Context.BATTERY_SERVICE) as? BatteryManager
            ?: return null
        val percent = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
            .coerceIn(0, 100)
        val status = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_STATUS)
        val isCharging = status == BatteryManager.BATTERY_STATUS_CHARGING ||
            status == BatteryManager.BATTERY_STATUS_FULL

        val powerManager = context.getSystemService(Context.POWER_SERVICE) as? PowerManager
        val powerSaveMode = powerManager?.isPowerSaveMode ?: false
        val thermalLevel = readThermalStatus(powerManager)

        return FfiPowerState(
            batteryPercent = percent.toUByte(),
            isCharging = isCharging,
            thermalLevel = thermalLevel,
            powerSaveMode = powerSaveMode
        )
    }

    /**
     * Reads the thermal status of the device.
     *
     * The API exists since Android 10. On older versions the device reports
     * no thermal state, which the core treats as "unknown" and not as a
     * restriction.
     */
    private fun readThermalStatus(powerManager: PowerManager?): Int {
        if (powerManager == null || Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return -1
        }
        return runCatching { powerManager.currentThermalStatus }.getOrDefault(-1)
    }
}
