//! Conversion of `db` errors into their wire mirrors in
//! [`common::abi`]. Only the failure kind crosses the boundary.

use common::abi;

pub(crate) trait ToAbi {
    type Out;
    fn to_abi(self) -> Self::Out;
}

impl ToAbi for db::NodeNotFoundError {
    type Out = abi::NodeNotFoundError;
    fn to_abi(self) -> Self::Out {
        abi::NodeNotFoundError
    }
}

impl ToAbi for db::EdgeNotFoundError {
    type Out = abi::EdgeNotFoundError;
    fn to_abi(self) -> Self::Out {
        abi::EdgeNotFoundError
    }
}

impl ToAbi for db::HyperedgeNotFoundError {
    type Out = abi::HyperedgeNotFoundError;
    fn to_abi(self) -> Self::Out {
        abi::HyperedgeNotFoundError
    }
}

impl ToAbi for db::NoAttachedObjectError {
    type Out = abi::NoAttachedObjectError;
    fn to_abi(self) -> Self::Out {
        abi::NoAttachedObjectError
    }
}

impl ToAbi for db::AddEdgeError {
    type Out = abi::AddEdgeError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::MissingEndpoints(_) => abi::AddEdgeError::MissingEndpoints,
            Self::EdgeAlreadyExists(_) => abi::AddEdgeError::EdgeAlreadyExists,
        }
    }
}

impl ToAbi for db::CreateHyperedgeError {
    type Out = abi::CreateHyperedgeError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::PointeesNotFound(_) => abi::CreateHyperedgeError::PointeesNotFound,
            Self::HyperedgeAlreadyExists(_) => abi::CreateHyperedgeError::HyperedgeAlreadyExists,
            Self::EmptyHyperedge => abi::CreateHyperedgeError::EmptyHyperedge,
        }
    }
}

impl ToAbi for db::RetargetError {
    type Out = abi::RetargetError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::EdgeNotFound(_) => abi::RetargetError::EdgeNotFound,
            Self::InvalidTarget(_) => abi::RetargetError::InvalidTarget,
        }
    }
}

impl ToAbi for db::AttachObjectError {
    type Out = abi::AttachObjectError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::AttachTargetNotFound(_) => abi::AttachObjectError::AttachTargetNotFound,
            Self::IncorrectType(_) => abi::AttachObjectError::IncorrectType,
        }
    }
}

impl ToAbi for db::GetEdgeError {
    type Out = abi::GetEdgeError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::NotFound(_) => abi::GetEdgeError::NotFound,
            Self::IncorrectType(_) => abi::GetEdgeError::IncorrectType,
        }
    }
}

impl ToAbi for db::AddHyperedgeMembersError {
    type Out = abi::AddHyperedgeMembersError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::HyperedgeNotFound(_) => abi::AddHyperedgeMembersError::HyperedgeNotFound,
            Self::MembersAlreadyExist(_) => abi::AddHyperedgeMembersError::MembersAlreadyExist,
            Self::PointeesNotFound(_) => abi::AddHyperedgeMembersError::PointeesNotFound,
        }
    }
}

impl ToAbi for db::RemoveHyperedgeMembersError {
    type Out = abi::RemoveHyperedgeMembersError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::HyperedgeNotFound(_) => abi::RemoveHyperedgeMembersError::HyperedgeNotFound,
            Self::MembersNotInHyperedge(_) => {
                abi::RemoveHyperedgeMembersError::MembersNotInHyperedge
            }
        }
    }
}

impl ToAbi for db::UndoError {
    type Out = abi::UndoError;
    fn to_abi(self) -> Self::Out {
        match self {
            Self::Apply(_) => abi::UndoError::Apply,
            Self::UnresolvedCascade(_) => abi::UndoError::UnresolvedCascade,
        }
    }
}
