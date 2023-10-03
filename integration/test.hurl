GET {{host}}/config/unknownCollection

HTTP 404
[Asserts]
body contains "unknownCollection"


GET {{host}}/config/secondCollection

HTTP 200
[Asserts]
header "Server" not exists
jsonpath "$[0]._id" == "_authorization"
jsonpath "$[1]._id" == "one"
jsonpath "$[1].some" == "value"
jsonpath "$[1].other" == 37.5
jsonpath "$[2]._id" == "two"
jsonpath "$[2].some" == "otherVal"
jsonpath "$[2].other" == 42.9


GET {{host}}/config/unknownCollection/unknownId

HTTP 404
[Asserts]
body contains "unknownCollection"
body contains "unknownId"


GET {{host}}/config/firstCollection/one

HTTP 200
[Asserts]
header "Server" not exists
jsonpath "$.first" == false
jsonpath "$.second" == 1


GET {{host}}/config/firstCollection/two

HTTP 200
[Asserts]
header "Server" not exists
jsonpath "$.first" == true
jsonpath "$.second" == 2


GET {{host}}/config/firstCollection/missing_target

HTTP 404
[Asserts]
body contains "firstCollection"
body matches /[0-9a-f]{24}/


GET {{host}}/config/secondCollection/one

HTTP 200
[Asserts]
header "Server" not exists
jsonpath "$.some" == "value"
jsonpath "$.other" == 37.5


GET {{host}}/config/secondCollection/two

HTTP 200
[Asserts]
header "Server" not exists
jsonpath "$.some" == "otherVal"
jsonpath "$.other" == 42.9


PATCH {{host}}/config/firstCollection/one
{
  "second": 5
}

HTTP 401


PATCH {{host}}/config/secondCollection/one
{
  "other": 850
}

HTTP 401


PATCH {{host}}/config/secondCollection/one
{
  "some": "changed"
}

HTTP 200


GET {{host}}/config/secondCollection/one

HTTP 200
[Asserts]
jsonpath "$.some" == "changed"
