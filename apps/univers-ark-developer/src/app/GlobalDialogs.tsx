import { AddMachineDialog } from "../components/AddMachineDialog";
import { ServerDialog } from "../components/ServerDialog";
import type { ManagedMachine } from "../types";

export type EditingMachineState = {
  initialTab: "general" | "connection" | "discovery" | "containers";
  machineId: string;
};

type DialogSaveEvent = {
  close?: boolean;
};

interface GlobalDialogsProps {
  defaultProfileId?: string;
  editingMachine: EditingMachineState | null;
  editingMachineRecord: ManagedMachine | null;
  isAddMachineDialogOpen: boolean;
  isCreatingMachine: boolean;
  onCloseAddMachineDialog: () => void;
  onCloseCreatingMachine: () => void;
  onCloseEditingMachine: () => void;
  onImportedMachine: () => void;
  onOpenCustomMachine: () => void;
  onSavedCreatedMachine: (event?: DialogSaveEvent) => void;
  onSavedEditedMachine: (event?: DialogSaveEvent) => void;
}

export function GlobalDialogs({
  defaultProfileId = "",
  editingMachine,
  editingMachineRecord,
  isAddMachineDialogOpen,
  isCreatingMachine,
  onCloseAddMachineDialog,
  onCloseCreatingMachine,
  onCloseEditingMachine,
  onImportedMachine,
  onOpenCustomMachine,
  onSavedCreatedMachine,
  onSavedEditedMachine,
}: GlobalDialogsProps) {
  return (
    <>
      {isAddMachineDialogOpen ? (
        <AddMachineDialog
          onClose={onCloseAddMachineDialog}
          onImported={onImportedMachine}
          onOpenCustom={onOpenCustomMachine}
        />
      ) : null}

      {isCreatingMachine ? (
        <ServerDialog
          defaultProfileId={defaultProfileId}
          onClose={onCloseCreatingMachine}
          onSaved={onSavedCreatedMachine}
        />
      ) : null}

      {editingMachineRecord && editingMachine ? (
        <ServerDialog
          defaultProfileId={defaultProfileId}
          initialTab={editingMachine.initialTab}
          onClose={onCloseEditingMachine}
          onSaved={onSavedEditedMachine}
          server={editingMachineRecord}
        />
      ) : null}
    </>
  );
}
