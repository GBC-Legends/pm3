import { useCallback } from 'react';
import { daemon } from '../services';
import { useApp } from '../context/AppContext';

export function useProcesses() {
  const { processes, refreshProcesses, addToast } = useApp();

  const startProcess = useCallback(async (id) => {
    try {
      const promise = daemon.startProcess(id);
      refreshProcesses();
      await promise;
      refreshProcesses();
      addToast('Process started', 'success');
    } catch (err) {
      addToast(err.message, 'error');
    }
  }, [refreshProcesses, addToast]);

  const stopProcess = useCallback(async (id) => {
    try {
      const promise = daemon.stopProcess(id);
      refreshProcesses();
      await promise;
      refreshProcesses();
      addToast('Process stopped', 'success');
    } catch (err) {
      addToast(err.message, 'error');
    }
  }, [refreshProcesses, addToast]);

  const restartProcess = useCallback(async (id) => {
    try {
      const promise = daemon.restartProcess(id);
      refreshProcesses();
      await promise;
      refreshProcesses();
      addToast('Process restarted', 'success');
    } catch (err) {
      addToast(err.message, 'error');
    }
  }, [refreshProcesses, addToast]);

  const deleteProcess = useCallback(async (id) => {
    try {
      await Promise.resolve(daemon.deleteProcess(id));
      refreshProcesses();
      addToast('Process deleted', 'success');
    } catch (err) {
      addToast(err.message, 'error');
    }
  }, [refreshProcesses, addToast]);

  const addProcess = useCallback(async (config) => {
    try {
      const promise = daemon.addProcess(config);
      refreshProcesses();
      await promise;
      refreshProcesses();
      addToast(`Process "${config.name}" started`, 'success');
    } catch (err) {
      addToast(err.message, 'error');
    }
  }, [refreshProcesses, addToast]);

  return { processes, startProcess, stopProcess, restartProcess, deleteProcess, addProcess };
}
