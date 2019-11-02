port module Main exposing (..)

import Browser
import Html exposing (Html, button, div, form, input, li, text, ul)
import Html.Attributes exposing (autofocus, class, classList, id, type_, value)
import Html.Events exposing (onClick, onInput, onSubmit)
import Json.Decode as Decode exposing (Decoder, field, map2)
import Json.Encode exposing (Value, bool, encode, int, object, string)


port toRust : Value -> Cmd msg


port fromRust : (Value -> msg) -> Sub msg


main =
    Browser.element
        { init = init
        , update = update
        , view = view
        , subscriptions = subscriptions
        }


type RustCommand
    = Init
    | Log { text : String }
    | AddTask { name : String }
    | MarkTask { index : Int, done : Bool }
    | ClearDoneTasks


encodeRustCommand : RustCommand -> Value
encodeRustCommand command =
    case command of
        Init ->
            object [ ( "cmd", string "Init" ) ]

        Log { text } ->
            object [ ( "cmd", string "Log" ), ( "text", string text ) ]

        AddTask { name } ->
            object [ ( "cmd", string "AddTask" ), ( "name", string name ) ]

        MarkTask { index, done } ->
            object [ ( "cmd", string "MarkTask" ), ( "index", int index ), ( "done", bool done ) ]

        ClearDoneTasks ->
            object [ ( "cmd", string "ClearDoneTasks" ) ]



-- MODEL


type alias Task =
    { name : String
    , done : Bool
    }


type alias Model =
    { str : String
    , field : String
    , tasks : List Task
    }


type Msg
    = UpdateField String
    | SendToRust RustCommand
    | UpdateTasks (List Task)


init : () -> ( Model, Cmd Msg )
init _ =
    ( { str = "", field = "", tasks = [] }, toRust (encodeRustCommand Init) )



----- UPDATE


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        UpdateTasks tasks ->
            ( { model | tasks = tasks }, Cmd.none )

        SendToRust command ->
            ( model, toRust (encodeRustCommand command) )

        UpdateField field ->
            ( { model | field = field }, Cmd.none )



-- VIEW


viewTask : Int -> Task -> Html Msg
viewTask index task =
    div
        [ classList
            [ ( "task-item", True )
            , ( "checked", task.done == True )
            , ( "unchecked", task.done == False )
            ]
        , onClick (SendToRust (MarkTask { index = index, done = not task.done }))
        ]
        [ text task.name ]


view : Model -> Html Msg
view model =
    div [ class "container" ]
        [ text model.str
        , form
            [ class "text-input-wrapper", onSubmit (SendToRust (AddTask { name = model.field })) ]
            [ input
                [ id "task-name-input"
                , class "text-input"
                , type_ "text"
                , autofocus True
                , value model.field
                , onInput UpdateField
                ]
                []
            ]
        , div [ class "task-list" ] (List.indexedMap viewTask model.tasks)
        , div [ class "footer" ]
            [ div [ class "btn-clear-tasks", onClick (SendToRust ClearDoneTasks) ] [ text "Delete completed" ]
            ]
        ]



-- SUBSCRIPTIONS


taskDecoder : Decoder Task
taskDecoder =
    map2 Task
        (field "name" Decode.string)
        (field "done" Decode.bool)


taskListDecoder : Decoder (List Task)
taskListDecoder =
    Decode.list taskDecoder


decodeValue : Value -> Msg
decodeValue x =
    let
        result =
            Decode.decodeValue taskListDecoder x
    in
    case result of
        Ok tasks ->
            UpdateTasks tasks

        Err err ->
            SendToRust (Log { text = Decode.errorToString err })


subscriptions : Model -> Sub Msg
subscriptions model =
    fromRust decodeValue
